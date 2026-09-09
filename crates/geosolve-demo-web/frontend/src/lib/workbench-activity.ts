// SPDX-License-Identifier: GPL-3.0-or-later

/** Presentation-only activity; it grants no scene or editing authority. */
export class WorkbenchActivity {
  private readonly pending = new Set<object>();
  private readonly listeners = new Set<() => void>();
  private readonly pendingListeners = new Set<() => void>();
  private timer?: ReturnType<typeof setTimeout>;
  private visible = false;

  readonly getSnapshot = () => this.visible;
  readonly getPendingSnapshot = () => this.pending.size > 0;
  readonly subscribePending = (listener: () => void) => {
    this.pendingListeners.add(listener);
    return () => { this.pendingListeners.delete(listener); };
  };
  readonly subscribe = (listener: () => void) => {
    this.listeners.add(listener);
    return () => { this.listeners.delete(listener); };
  };

  begin(): () => void {
    const token = {};
    this.pending.add(token);
    if (this.pending.size === 1) {
      for (const listener of this.pendingListeners) listener();
      this.timer = setTimeout(() => {
        this.timer = undefined;
        this.setVisible(true);
      }, 500);
    }
    return () => {
      if (!this.pending.delete(token) || this.pending.size) return;
      for (const listener of this.pendingListeners) listener();
      clearTimeout(this.timer);
      this.timer = undefined;
      this.setVisible(false);
    };
  }

  async track<T>(action: () => T | Promise<T>): Promise<T> {
    const finish = this.begin();
    try { return await action(); } finally { finish(); }
  }

  /** Retire disconnected/disposed work; late completions cannot clear newer work. */
  reset(): void {
    const wasPending = this.getPendingSnapshot();
    this.pending.clear();
    if (wasPending) for (const listener of this.pendingListeners) listener();
    clearTimeout(this.timer);
    this.timer = undefined;
    this.setVisible(false);
  }

  private setVisible(visible: boolean) {
    if (visible === this.visible) return;
    this.visible = visible;
    for (const listener of this.listeners) listener();
  }
}
