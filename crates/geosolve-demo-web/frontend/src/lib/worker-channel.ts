// SPDX-License-Identifier: GPL-3.0-or-later

export type WorkerTransport = Pick<Worker, "postMessage" | "addEventListener" | "removeEventListener" | "terminate">;
export type WithoutWorkerId<T> = T extends unknown ? Omit<T, "id"> : never;
export interface WorkerEnvelope { readonly id: number; readonly generation?: number }
interface Waiter<T> { resolve(value: T): void; reject(error: Error): void }
interface Pending<Input, Result> extends Waiter<Result> { readonly input: Input; readonly generation?: number }
export const workerError = (error: unknown) => error instanceof Error ? error : Error(String(error));

export interface WorkerChannelOptions<Input, Response, Result> {
  readonly stoppedMessage: string;
  readonly unreadableMessage: string;
  /** Domain refusals reject one request. A malformed successful reply retires the channel. */
  readonly responseError: (response: Response) => string | undefined;
  readonly decode: (response: Response, input: Input) => Result;
  readonly postFailure?: "request" | "channel";
  readonly onFailure?: (error: Error) => void;
}

/** Request correlation and worker lifetime only. Callers retain their queue policies. */
export class WorkerRequestChannel<Input extends object, Response extends WorkerEnvelope, Result> {
  private nextId = 0;
  private readonly pending = new Map<number, Pending<Input, Result>>();
  private closed?: Error;
  get failure() { return this.closed; }
  constructor(private readonly worker: WorkerTransport, private readonly options: WorkerChannelOptions<Input, Response, Result>) {
    worker.addEventListener("message", this.receive);
    worker.addEventListener("error", this.error);
    worker.addEventListener("messageerror", this.messageError);
  }
  request<T extends Result = Result>(input: Input): Promise<T> {
    return new Promise((resolve, reject) => this.send(input, { resolve: resolve as (value: Result) => void, reject }));
  }
  /** Synchronous settlement keeps mailbox admission independent of Promise scheduling. */
  send(input: Input, waiter: Waiter<Result>) {
    if (this.closed) { waiter.reject(this.closed); return; }
    const id = ++this.nextId;
    const generation = "generation" in input ? input.generation as number : undefined;
    this.pending.set(id, { ...waiter, input, generation });
    try { this.worker.postMessage({ ...input, id }); }
    catch (error) {
      const failure = workerError(error);
      if (this.options.postFailure === "channel") this.fail(failure);
      else { this.pending.delete(id); waiter.reject(failure); }
    }
  }
  /** Retire requests without reusing IDs or granting an old reply a new lifetime. */
  invalidate(error: Error) {
    const pending = [...this.pending.values()];
    this.pending.clear();
    for (const waiter of pending) waiter.reject(error);
  }
  fail(error: Error) {
    if (this.closed) return;
    this.closed = error;
    this.worker.removeEventListener("message", this.receive);
    this.worker.removeEventListener("error", this.error);
    this.worker.removeEventListener("messageerror", this.messageError);
    this.worker.terminate();
    this.invalidate(error);
    this.options.onFailure?.(error);
  }
  private receive = ({ data }: MessageEvent<Response>) => {
    const pending = this.pending.get(data?.id);
    if (!pending || pending.generation !== undefined && data.generation !== pending.generation) return;
    this.pending.delete(data.id);
    try {
      const error = this.options.responseError(data);
      if (error !== undefined) { pending.reject(Error(error)); return; }
      pending.resolve(this.options.decode(data, pending.input));
    } catch (error) {
      const failure = workerError(error);
      pending.reject(failure);
      this.fail(failure);
    }
  };
  private error = (event: ErrorEvent) => { event.preventDefault(); this.fail(Error(event.message || this.options.stoppedMessage)); };
  private messageError = () => this.fail(Error(this.options.unreadableMessage));
}

interface Queued<Action, Result> { action: Action; readonly generation: number; readonly waiters: Waiter<Result>[] }
export interface WorkerMailboxOptions<Action, Response, Result> extends WorkerChannelOptions<Action & { generation: number }, Response, Result> {
  readonly unopenedMessage: string;
  readonly replacedMessage: string;
  readonly fullMessage: string;
  readonly limit: number;
  /** Only adjacent queued actions may combine; running work is never rewritten. */
  readonly combine: (previous: Action, next: Action) => Action | undefined;
}

/** One running request per model generation, with an explicit owner-supplied merge policy. */
export class WorkerMailbox<Action extends object, Response extends WorkerEnvelope, Result> {
  private readonly channel: WorkerRequestChannel<Action & { generation: number }, Response, Result>;
  private generation = 0;
  private queued: Queued<Action, Result>[] = [];
  private running?: Queued<Action, Result>;
  private scheduled = false;
  private count = 0;
  constructor(worker: WorkerTransport, private readonly options: WorkerMailboxOptions<Action, Response, Result>) {
    this.channel = new WorkerRequestChannel(worker, { ...options, postFailure: "channel", onFailure: (error) => {
      this.invalidate(error);
      options.onFailure?.(error);
    } });
  }
  replace<T extends Result = Result>(action: Action): Promise<T> {
    this.invalidate(Error(this.options.replacedMessage));
    ++this.generation;
    return this.request<T>(action);
  }
  request<T extends Result = Result>(action: Action): Promise<T> {
    if (this.channel.failure) return Promise.reject(this.channel.failure);
    if (!this.generation) return Promise.reject(Error(this.options.unopenedMessage));
    if (this.count >= this.options.limit) return Promise.reject(Error(this.options.fullMessage));
    return new Promise((resolve, reject) => {
      const waiter = { resolve: resolve as (value: Result) => void, reject };
      const previous = this.queued.at(-1);
      const combined = previous?.generation === this.generation ? this.options.combine(previous.action, action) : undefined;
      if (combined !== undefined) { previous!.action = combined; previous!.waiters.push(waiter); }
      else this.queued.push({ action, generation: this.generation, waiters: [waiter] });
      ++this.count;
      this.schedule();
    });
  }
  dispose(error: Error) { this.channel.fail(error); }
  private schedule() {
    if (this.scheduled || this.running || this.channel.failure) return;
    this.scheduled = true;
    queueMicrotask(() => { this.scheduled = false; this.pump(); });
  }
  private pump() {
    if (this.running || this.channel.failure) return;
    const next = this.queued.shift();
    if (!next) return;
    this.running = next;
    const settle = (deliver: (waiter: Waiter<Result>) => void) => {
      if (this.running !== next) return;
      this.running = undefined; this.count -= next.waiters.length;
      next.waiters.forEach(deliver);
      this.schedule();
    };
    this.channel.send({ ...next.action, generation: next.generation }, {
      resolve: (result) => settle((waiter) => waiter.resolve(result)),
      reject: (error) => settle((waiter) => waiter.reject(error)),
    });
  }
  private invalidate(error: Error) {
    const pending = [...this.queued, ...(this.running ? [this.running] : [])];
    this.queued = []; this.running = undefined; this.count = 0;
    this.channel.invalidate(error);
    for (const request of pending) for (const waiter of request.waiters) waiter.reject(error);
  }
}
