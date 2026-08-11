# MyLore — Design System

> Phase 0 · August 2026 · Basis: `UX_RESEARCH.md` (Design A base + C affordances)
> Rules: tokens, not ad-hoc values; dark + light from day one; RTL via logical properties.

---

## 1. Principles

1. **Calm default, power on demand** — quiet surfaces; density and keyboard are features.
2. **Tokens everywhere** — no magic colors/spacings in components.
3. **Logical properties** (`padding-inline`, `inset-inline-start`) → RTL works automatically.
4. **Covers are the content** — UI chrome is restrained so artwork carries the page.
5. **WCAG 2.1 AA** contrast in both themes; focus always visible.

## 2. Color tokens

| Token | Light | Dark | Usage |
|---|---|---|---|
| `bg-base` | `#FAFAF9` | `#141417` | app background |
| `bg-surface` | `#FFFFFF` | `#1C1C21` | cards, panels |
| `bg-raised` | `#FFFFFF` | `#24242A` | popovers, dialogs |
| `bg-hover` | `#F1F1EF` | `#2A2A31` | hover |
| `border-subtle` | `#E5E4E0` | `#2E2E36` | hairline borders |
| `border-strong` | `#D4D3CE` | `#3A3A44` | inputs, focus ring base |
| `text-primary` | `#1C1B1A` | `#F2F1EF` | body |
| `text-secondary` | `#57564F` | `#A8A7A0` | secondary |
| `text-tertiary` | `#85847C` | `#71716C` | captions |
| `accent` | `#B4541F` | `#E08A4C` | primary actions, progress |
| `accent-hover` | `#9C4719` | `#EEA060` | hover accent |
| `accent-soft` | `#F7E7DC` | `#3A2A1F` | selected/active bg |
| `ok` | `#2F7D32` | `#7BC47F` | completed, success |
| `warn` | `#9A6700` | `#D9A441` | on-hold, warnings |
| `danger` | `#C62828` | `#E57373` | destructive |
| `info` | `#1565C0` | `#64B5F6` | info |

Accent rationale: covers are vivid; a warm amber-bronze reads as "library/reading" rather than
app-y blue and holds in both themes. Status→color mapping is tokenized
(`status-planned/-inprogress/-completed/-onhold/-dropped/-repeat`) so custom statuses map
consistently.

## 3. Typography

- **UI font:** system stack (Segoe UI / SF / Roboto) — native feel, zero download. Arabic/CJK
  fallbacks come from the OS (optional Cairo/Noto override in preferences).
- Scale (rem): `12/13/14/16/20/28/36`. Body 14px; secondary 13px; tables 13px.
- Numerals: `font-variant-numeric: tabular-nums` for progress/stats.
- Long titles: `line-clamp: 2` on cards; full title on hover/detail; no layout shift.

## 4. Spacing, radius, elevation

- Spacing scale (px): `4/8/12/16/24/32/48` (4px unit).
- Radius: `sm 6 · md 10 · lg 16 · full 999`. Cards `md`; buttons `sm`; dialogs `lg`.
- Elevation: 1) hairline border; 2) `shadow-sm` on hover; 3) `shadow-lg` for menus/dialogs.
  Flat-first; shadows only for floating elements.
- Focus ring: 2px `accent` with 3px transparent gap (visible in both themes).

## 5. Icons

- **Lucide** (MIT), stroke 1.75, sizes 16/20/24. Single icon family; no emoji in UI.
- Icon + text for primary nav; icon-only with tooltips for toolbar actions (tooltips always
  populated — a11y).

## 6. Components

| Component | Notes |
|---|---|
| Button | variants: primary / secondary / ghost / danger; sizes sm/md; icon-only allowed with aria-label |
| Input / Textarea / Select / Combobox | shared field tokens; inline validation slot; label required |
| Checkbox / Radio / Switch | Radix primitives, RTL-aware |
| Card | cover (2:3) + title (2 lines) + status badge + thin progress bar + hover quick-actions |
| Row (list item) | thumb, titles, meta, inline progress, kebab menu |
| Badge / Chip | status, genre, tag, external-provider |
| Progress bar | 4px, accent→ok when complete; on-card overlay for grid |
| Tabs / Toggle group | detail page sections |
| Table | virtualized, sticky header, row selection, numeric alignment |
| Dialog | Radix; focus trap, `Esc` close, `dir` aware |
| Popover / Context menu | Radix; keyboard-openable |
| Command palette | custom; shows shortcut hints per action |
| Toast | success/error/undo-action; auto-dismiss, pause on hover |
| Skeleton | matches final layout; shimmer off under reduced-motion |
| Tooltip | always has text; 400ms delay |
| Empty state | icon + title + hint + primary action |
| Pagination | only where virtualization is impractical |

## 7. Patterns & motion

- Micro-interactions only where they aid clarity: row hover, card lift, dialog enter, toast
  slide. No decorative animation loops.
- Duration 120–200ms ease-out; reduced-motion disables transforms entirely.
- Progress updates animate only when jumping is small (<20% delta) to avoid flicker on large libs.

## 8. Density tiers

- **Comfortable** (default) / **Compact** (user setting). Compact reduces paddings (8→4),
  font 14→13, card art size, and enables the dense List/Compact list views.

## 9. Dark & light mode

- CSS custom properties per theme on `:root[data-theme=light|dark]`; components consume tokens
  only. System preference is the default; user override persists.
- Both themes pass AA; accent/success/warn/danger pairs chosen for that.

## 10. RTL support

- Layout via logical props + Tailwind `rtl:` variants; nav rail mirrors, back arrows flip,
  progress bars fill from inline-start.
- Mixed-direction titles are left as-is (per-title `dir` attr by script detection); UI chrome is
  fully mirrored. Test matrix: LTR, RTL, long titles, Arabic, mixed AR+EN, JP/ZH/KO (REQ-UX-003).

## 11. Accessibility checklist (per component)

- Keyboard operable (tab, arrows, enter, esc) · focus visible · focus trapped in dialogs ·
  labels/aria on all controls · contrast AA · reduced-motion respected · tooltips not sole
  affordance · tables expose row context.

