// SPDX-License-Identifier: GPL-3.0-or-later
/** A domain worker never exports baked profiles, so it owns exactly the current
 * accepted result of each live native session. Engine result ownership is
 * independent of session lifetime and must be released explicitly.
 */
export function createDomainSessionOwner(engine) {
  const live = new Set();
  return {
    open(project, options) {
      const session = engine.openEditableSession(project, options); live.add(session); return session;
    },
    async change(session, action) {
      if (!live.has(session)) throw Error("Domain session is not live");
      const before = session.accepted;
      try { return await action(); }
      finally { if (session.accepted !== before) engine.release(before); }
    },
    close(session) {
      if (!live.delete(session)) return;
      const result = session.accepted;
      try { session.dispose(); } finally { engine.release(result); }
    },
  };
}
