# Settings

## Summary

Imp Lab's **Settings** dialog (Edit → Settings…, `Ctrl+,`) shows settings that extensions declare. Settings, settings categories, and the associations between them are three **independent** component kinds in `extensions.json` (see [extensions](extensions.md#settings-components)):

- A **setting** is registered on its own, with no page.
- A **settings category** is thin: an id and a user-friendly title. Each category is one settings page.
- A **settings-category-item** associates one setting with one category. Any extension may associate any setting with any category, including another extension's.

Lab draws settings without knowing what they mean. Values are opaque JSON: Lab stores them and delivers them to the service that owns each setting.

## Pages

- One page per enabled category, ordered by title. Within a page, settings are ordered by each association's `order`, then by label.
- A setting associated with several categories appears on each of their pages; it is the same value.
- **Fallback page.** Settings that no valid association places go on an **Other** page, listed last and shown only when it has settings.
- An association naming a missing setting or category is ignored and logged (`get_settings_layout` warnings). The setting then falls back to Other.
- A status item can open the dialog straight at a category.

## Value types

| `valueType` | Control | Value |
| --- | --- | --- |
| `choice` | Select | a string from the options |
| `multi-choice` | Chip group (honors `minSelected`) | an array of strings from the options |
| `boolean` | Switch | `true` / `false` |

- Options are static (`params.options: [{value, label}]`) or live from a topic (`params.optionsTopic`), which updates while the dialog is open.
- A saved `choice` value missing from the current options stays selected and shows as "‹value› (unavailable)". An unplugged device stays chosen, for example.
- A value that doesn't fit its declaration falls back to the declared `default`.
- Number and text types are not supported yet.

## Storage and delivery

- Values live in the `extension-settings` user-state entry (`settings` bucket), as `{settingId: value}`; see [user state](user-state.md).
- Changes apply immediately (no OK / Apply step) and are saved.
- Each owning service (`params.serviceId`, a service component id) gets its settings' values as one service `event`: `{"type":"settings","values":{…}}`. That happens once after startup and again on every change to one of its settings.

## Planned

- Searching across settings (labels and descriptions are already data).
- Lab's own built-in preferences as settings.
