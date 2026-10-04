# Secopy design system

How the app's screens are built, so they look and behave the same. The colours come from
[RFD §5.6](../rfd/0001-secopy.md); everything else is defined here. Issue #22.

To see every component and screen: `cd ui && npm run dev`, then open
<http://localhost:5173/gallery.html> (add `#setup`, `#progress`, `#summary`, `#settings`,
`#presets`, `#presets-empty`, `#queue`, `#queue-summary`, `#mirror`, `#mirror-preview`,
`#mirror-summary`, `#mirroring`, `#verify`, `#verify-summary`, `#import`, `#export`, `#tabs`, `#cancel`, `#removing`, `#panel`, `#panel-done` or `#components` for a screen; `?tips` shows every `Hint`
and every button's help, `?help` only the buttons' help, `?help=primary` only the primary button's, as in
`gallery.html?help=primary#setup`). The
gallery is dev only and not in the app.

## Layout

The kinds of job (Copy, Mirror, Verify) are tabs in a `TabBar` at the top of the section
screens: plain words, the selected one underlined in the accent. On its right, apart from the
tabs, the Queue (a button with its job count, highlighted while it's open) and Settings, once
for the whole app. The bar is hidden while jobs run and on Settings and Copy presets, so the width
goes to paths and file lists. Below it, every screen is an `AppShell` with three parts,
always in the same place:

```
┌──────────────────────────────────────────────────────────────┐
│  Copy   Mirror   Verify             [☰ Queue 3] [⚙ Settings] │  tab bar (sections)
│  ‾‾‾‾                                                        │
├──────────────────────────────────────────────────────────────┤
│ Screen title                                                 │  header
├──────────────────────────────────────────────────────────────┤
│  Sections (FROM, TO, Files…): the only part that scrolls     │  content
├──────────────────────────────────────────────────────────────┤
│ [‹ Back] [other] [danger]     status          [ PRIMARY ]   │  action bar
└──────────────────────────────────────────────────────────────┘
```

- **Header** (`ScreenHeader`): the screen's title (the window's title bar already says Secopy), so you
  know where you are inside a tab (New copy, Summary, Preview…); a screen's own controls on the right.
- **Content**: `Section`s, one per part of the screen, and app-wide messages first.
  Inside a section, one `FormRow` per kind of thing (Source, Preset, Options,
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
  and Save (right, on only when something changed; it saves and goes back). A copy preset has
  Revert and Save, and leaving it with unsaved changes asks first.

## Tokens (`ui/src/app.css`)

| Kind | Tokens |
|---|---|
| Colour | `bg`, `surface`, `surface-raised`, `border`, `text`, `text-muted`, `text-faint`, `accent`, `accent-soft` (chosen items), `accent-strong` (filled accent backgrounds: primary button, selected segment), `accent-strong-hover` (a filled button under the pointer), `on-accent`, `success`, `warning`, `danger` |
| Spacing | `space-1` 4 · `space-2` 8 · `space-3` 12 · `space-4` 16 · `space-5` 24 px |
| Type | `text-xs` 11 (section labels, caps) · `text-sm` 12 (help, meta) · `text-md` 14 (body) · `text-lg` 16 (screen titles) · `text-xl` 20 (result headlines); the macOS system font; tabular numbers; `font-mono` for paths and hashes |
| Shape | `radius-control` 6 · `radius` 8 (sections) · `radius-pill`; `control-height` 30 px |
| Motion | `duration` 150 ms, `ease`; none with reduced motion |

Components use tokens only, never raw colours or sizes.

## Components (`ui/src/lib/ui/`)

| Component | Use it for |
|---|---|
| `AppShell` | Every screen's frame: `header`, content, `actions` |
| `ScreenHeader` | Title and optional trailing controls |
| `TabBar` | The kinds of job as underlined tabs at the top (Copy, Mirror, Verify); on the right the Queue button (its count, highlighted while open) and Settings; hidden while jobs run and on Settings/Copy presets |
| `ActionBar` | The bottom bar: `start`, `status`, `end` |
| `Section` | A titled part of a screen; the only card style |
| `FormRow` | One labelled line inside a section: the label column on the left, the content, the row's own actions on the right (Choose…, All · None) |
| `Button` | `primary` (one per screen), `secondary`, `danger`, `link`; optional icon; optional `help`: one sentence in a tip above the button, on hover (after half a second, like a macOS help tag) and keyboard focus, read by VoiceOver as its description; lined up with the button's right edge when it would run off the window; none while the button is disabled. Pass `""` while the words aren't known yet, so the button isn't rebuilt when they come |
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
| `Icon` | A few Lucide icons, always next to words; the one exception is the Queue's drag grip, a quiet button with a label and a help tag |
| `Hint` | A term that isn't clear on its own, explained on hover and keyboard focus: dotted underline, or an ⓘ mark with no term. `FormRow` (`hint`) and `Stats` items take one |
| `Dialog` | A question over the screen: safe answer first and focused, the other on the right; Esc is the safe answer. `api.confirm` shows one through `ConfirmHost`; Export's choice of what goes in the file (`ExportDialog`) is another |
| `IgnoreList` | A list of name patterns with a field to add one and a remove button per row (Settings, the preset editors, New copy's Also ignore) |

## Rules

- One primary button per screen, in the action bar on the right.
- The menu bar icon (#80) is a monochrome template glyph (two overlapping squares, drawn by
  `scripts/menubar-icon.py`) so macOS tints it. Clicking it opens the menu bar panel
  (`menubar/Panel.svelte`, 340 × 190, a see-through window showing a card with 12 px rounded
  corners and the native shadow, like a macOS popover; gallery `#panel` and `#panel-done`): the job, From/To
  paths cut at their start, a progress bar, files · speed · time left, Pause and Open Secopy,
  and Quit Secopy… as a link.
- Button labels use the fewest words that can't be read two ways: **Start** for every job
  (the tab or screen says which), **Update**, **Save as…**, **Clear…**, **Retry**. Keep the
  object when one word would be ambiguous (**Add to queue** next to the Queue button,
  **Save report…**) or when macOS has a standard phrase (**Show in Finder**). Dialog buttons
  name the action ("Cancel job" / "Continue"), never Yes / No.
- Colour never carries meaning alone: every status has an icon or a word.
- Chosen items are filled (`accent-soft`), not only outlined.
- White text sits on `accent-strong`, never on `accent` (3.2:1 fails WCAG AA); `accent` is for links, borders, focus rings and bars.
- A new screen focuses its title (`ScreenHeader` does it), so VoiceOver says where you are.
- Every field has a visible label; its error appears right under it and is linked to it.
- The user-facing words are "directory" and "file".
- Words live in `ui/src/locales/en.json` (and their Spanish in `es.json`); components use
  `t()`, never written text (a test checks). The terms to use are in [docs/i18n.md](../i18n.md). Rust sends codes (`Message { key, args }`, built with `msg!`); the UI shows them with
  `say()`. A form error's key is `errors.field.<field>.…`, so the editor puts it under its field.
- No emoji as icons.
- A `Hint` only where a word isn't clear on its own (Small files, Existing files, Same size and date, ignored files, Copy & Verify, archived, not started), in one or two plain sentences. Not for what a help line under an option already says.
- A button's `help` only where its short label hides the detail: Start (what it copies, where, the
  shortcut), Update, Save as…, Add to queue, Retry, Clear…, Pause / Resume, Cancel. One plain
  sentence, with the screen's real figures ("Copies 106 files (180.0 GB) to /Volumes/V001/Day01/CLIP
  and verifies them (⌘↩).") and a shortcut only where it works there (⌘↩ on New copy, ⌘. and Space
  while a job runs). Not on buttons that say it all: Back, Done, Choose…, Show in Finder, Save
  report…, Preview….
- Questions are asked in the window, never in a system alert (#113): `api.confirm` for a yes/no question, `Dialog` directly when it needs more (a checkbox, a choice).
