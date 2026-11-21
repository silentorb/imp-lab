/** Host input bindings: resolve KeyboardEvent → command id (VS Code–aligned). */

export type BindingDto = {
  command: string;
  key: string;
  modifiers: string[];
};

export type InputBindingsDto = {
  bindings: BindingDto[];
};

const NAMED_KEYS: Record<string, string> = {
  escape: "escape",
  esc: "escape",
  enter: "enter",
  return: "enter",
  tab: "tab",
  space: " ",
  spacebar: " ",
  backspace: "backspace",
  delete: "delete",
  del: "delete",
  arrowup: "arrowup",
  up: "arrowup",
  arrowdown: "arrowdown",
  down: "arrowdown",
  arrowleft: "arrowleft",
  left: "arrowleft",
  arrowright: "arrowright",
  right: "arrowright",
  home: "home",
  end: "end",
  pageup: "pageup",
  page_up: "pageup",
  pagedown: "pagedown",
  page_down: "pagedown",
  f1: "f1",
  f2: "f2",
  f3: "f3",
  f4: "f4",
  f5: "f5",
  f6: "f6",
  f7: "f7",
  f8: "f8",
  f9: "f9",
  f10: "f10",
  f11: "f11",
  f12: "f12",
};

type ModifierSet = {
  ctrl: boolean;
  alt: boolean;
  shift: boolean;
  logo: boolean;
};

function normalizeModifiers(names: string[]): ModifierSet {
  const set: ModifierSet = {
    ctrl: false,
    alt: false,
    shift: false,
    logo: false,
  };
  for (const name of names) {
    switch (name.toLowerCase()) {
      case "ctrl":
      case "control":
        set.ctrl = true;
        break;
      case "alt":
        set.alt = true;
        break;
      case "shift":
        set.shift = true;
        break;
      case "logo":
      case "super":
      case "meta":
      case "cmd":
      case "command":
        set.logo = true;
        break;
      default:
        break;
    }
  }
  return set;
}

function eventModifiers(event: KeyboardEvent): ModifierSet {
  // JSON `ctrl` matches Ctrl or Meta (Cmd) so Cmd+S works on macOS
  // alongside CmdOrCtrl menu accelerators.
  return {
    ctrl: event.ctrlKey || event.metaKey,
    alt: event.altKey,
    shift: event.shiftKey,
    logo: false,
  };
}

function modifiersEqual(a: ModifierSet, b: ModifierSet): boolean {
  return (
    a.ctrl === b.ctrl &&
    a.alt === b.alt &&
    a.shift === b.shift &&
    a.logo === b.logo
  );
}

function normalizeBindingKey(key: string): string {
  const lower = key.trim().toLowerCase();
  if (NAMED_KEYS[lower] !== undefined) {
    return NAMED_KEYS[lower];
  }
  return lower;
}

function eventKeyToken(event: KeyboardEvent): string | null {
  if (event.key === " ") {
    return " ";
  }
  const named = NAMED_KEYS[event.key.toLowerCase()];
  if (named !== undefined) {
    return named;
  }
  if (event.key.length === 1) {
    return event.key.toLowerCase();
  }
  return null;
}

/**
 * Resolve a keydown event against the host bindings table.
 * Exact modifier match; `ctrl` in JSON matches ctrlKey || metaKey.
 */
export function resolveKeyEvent(
  event: KeyboardEvent,
  bindings: BindingDto[],
): string | null {
  const eventKey = eventKeyToken(event);
  if (eventKey === null) {
    return null;
  }
  const mods = eventModifiers(event);

  // Last duplicate trigger wins (same as Rust load order).
  let matched: string | null = null;
  for (const binding of bindings) {
    if (normalizeBindingKey(binding.key) !== eventKey) {
      continue;
    }
    if (!modifiersEqual(normalizeModifiers(binding.modifiers), mods)) {
      continue;
    }
    matched = binding.command;
  }
  return matched;
}
