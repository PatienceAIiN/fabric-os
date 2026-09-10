# Design system — "Lumen" (M39)

An original visual identity for the ai-native-os desktop. No proprietary
assets, icons, layouts, or fonts are copied. Tokens live in `design/tokens.json`
and are the single source of styling; applications must not hard-code styles.

## Principles
- Familiar, modern, minimal, fast. Content first, chrome second.
- One accent, generous spacing, soft elevation. Light/dark/auto.
- Color is never the only status signal (accessibility): AI local/cloud also
  carry a filled-dot vs cloud glyph and a text label.

## Components (spec §40)
Desktop, top/system bar, dock, launcher, window manager, notification center,
quick settings, search, file manager, settings, terminal, system monitor, AI
assistant, AI activity panel, workspace manager. The terminal shell (`aios`)
implements command bar / activity / monitor today; the graphical widgets are
specified here for the compositor phase.

## AI indicators (spec §15)
- Local:  ● (success color) + "Local AI"
- Cloud:  ☁ (accent color) + "Claude"
High-risk AI actions use the danger color AND a ⛔ glyph AND a text reason.

## Performance budgets (spec §61)
Input < 16 ms, 60 FPS, immediate launch feedback, no blocking network on
settings navigation. Measured on reference hardware; not assumed universal.

## Motion & accessibility
Motion tokens are subtle and purposeful; `prefers-reduced-motion` collapses
durations to 0. Full keyboard navigation, high-contrast-friendly tokens, and
font scaling are required of every widget.
