# Secopy design system

How the app's screens are built, so they look and behave the same. The colours come from
[RFD §5.6](../rfd/0001-secopy.md); everything else is defined here. Issue #22.

To see every component and screen: `cd ui && npm run dev`, then open
<http://localhost:5173/gallery.html> (add `#setup`, `#progress`, `#summary`, `#settings`,
`#profiles` or `#profiles-empty` for a screen). The gallery is dev only and not in the app.

## Layout

The app's sections (Copy, Queue) are in a `Sidebar` on the left of the section screens; it
is hidden while jobs run and on Settings and Profiles. Every screen is an `AppShell` with
three parts, always in the same place:

```
┌──────────────────────────────────────────────────────────────┐
│ Screen title                                   [⚙ Settings] │  header
├──────────────────────────────────────────────────────────────┤
│  Sections (FROM, TO, Files…): the only part that scrolls     │  content
├──────────────────────────────────────────────────────────────┤
│ [‹ Back] [other] [danger]     status          [ PRIMARY ]   │  action bar
└──────────────────────────────────────────────────────────────┘
```

- **Header** (`ScreenHeader`): the screen's title (the window's title bar already says Secopy); screen-wide controls
  on the right (Settings on New copy and Summary).
- **Content**: `Section`s, one per part of the screen, and app-wide messages first.
  Inside a section, one `FormRow` per kind of thing (Source, Profile, Options,
  File types…), with a hairline between rows; a control whose row already names it hides
  its own label (`hideLabel`), keeping it for screen readers.
- **Action bar** (`ActionBar`), always visible:
  - **right:** the screen's one primary action (Start, New copy, Save);
  - **left:** ‹ Back first, when there is somewhere to go back to; then the other actions,
    and destructive ones (Cancel, Delete…) in red. Buttons that act on the screen are all
    here, never in the header;
  - **middle:** a short status. It explains a disabled primary action ("Choose where to
    copy to."), or says what will happen.
- Changes are saved explicitly, never on their own. Settings has Cancel (left; Esc too)
  and Save (right, on only when something changed; it saves and goes back). A profile has
  Revert and Save, and leaving it with unsaved changes asks first.

## Tokens (`ui/src/app.css`)

| Kind | Tokens |
|---|---|
| Colour | `bg`, `surface`, `surface-raised`, `border`, `text`, `text-muted`, `text-faint`, `accent`, `accent-soft` (chosen items), `accent-strong` (filled accent backgrounds: primary button, selected segment), `accent-strong-hover` (a filled button under the pointer), `on-accent`, `success`, `warning`, `danger` |
| Spacing | `space-1` 4 · `space-2` 8 · `space-3` 12 · `space-4` 16 · `space-5` 24 · `space-6` 32 px |
| Type | `text-xs` 11 (section labels, caps) · `text-sm` 12 (help, meta) · `text-md` 14 (body) · `text-lg` 16 (screen titles) · `text-xl` 20 (result headlines); the macOS system font; tabular numbers; `mono` for paths and hashes |
| Shape | `radius-control` 6 · `radius` 8 (sections) · `radius-pill`; `control-height` 30 px |
| Motion | `duration` 150 ms, `ease`; none with reduced motion |

Components use tokens only, never raw colours or sizes.

## Components (`ui/src/lib/ui/`)

| Component | Use it for |
|---|---|
| `AppShell` | Every screen's frame: `header`, content, `actions` |
| `ScreenHeader` | Title and optional trailing controls |
| `Sidebar` | The app's sections (Copy, Queue; Mirror later), with a count; hidden while jobs run and on Settings/Profiles |
| `ActionBar` | The bottom bar: `start`, `status`, `end` |
| `Section` | A titled part of a screen; the only card style |
| `FormRow` | One labelled line inside a section: the label column on the left, the content, the row's own actions on the right (Choose…, All · None) |
| `Button` | `primary` (one per screen), `secondary`, `danger`, `link`; optional icon |
| `SegmentedControl` | A small exclusive choice shown as one control (Copy / Copy & Verify) |
| `Checkbox` | An option, with an optional line of help |
| `RadioGroup` | Exclusive options under a legend |
| `TextField` | Labelled text input with help and an error tied to it; optional trailing button |
| `Select` | A labelled menu |
| `Chip` | A toggle with ✓ (file types) or a removable value |
| `Notice` | A message with icon and words: `info`, `success`, `warning`, `danger` (only danger interrupts) |
| `Stats` | Figures in one line, "3 files · 7.0 GB written · took 0:06" |
| `ProgressBar` | One phase's progress with speed and ETA |
| `EmptyState` | What an empty part is for and how to fill it |
| `Icon` | A few Lucide icons, always next to words |

## Rules

- One primary button per screen, in the action bar on the right.
- Colour never carries meaning alone: every status has an icon or a word.
- Chosen items are filled (`accent-soft`), not only outlined.
- White text sits on `accent-strong`, never on `accent` (3.2:1 fails WCAG AA); `accent` is for links, borders, focus rings and bars.
- A new screen focuses its title (`ScreenHeader` does it), so VoiceOver says where you are.
- Every field has a visible label; its error appears right under it and is linked to it.
- The user-facing words are "directory" and "file".
- No emoji as icons.
