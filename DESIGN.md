---
name: QuotaPulse
description: Monitor de cuotas de IA para Windows — tinta sobre papel, compacto y honesto
colors:
  paper: "#F4F3EF"
  paper-soft: "#FAF9F6"
  sheet: "#FFFFFF"
  rail: "#ECEAE4"
  hairline: "#E4E1DA"
  ink: "#111111"
  ink-secondary: "#4A4843"
  ink-muted: "#6F6C64"
  ink-block: "#111111"
  ink-block-text: "#F4F3EF"
  ink-block-muted: "#A9A69E"
  estimate-grey: "#A8A59D"
  live-green: "#23784A"
  warn-ochre: "#A15C00"
  danger-red: "#B42318"
  charcoal: "#141413"
  charcoal-sheet: "#1C1C1A"
  charcoal-raised: "#252523"
  charcoal-rail: "#2E2D2A"
  charcoal-active: "#3A3936"
  chalk: "#EDEBE6"
  chalk-secondary: "#B5B2AA"
  chalk-muted: "#8A877F"
  live-green-dark: "#5CC28A"
  warn-ochre-dark: "#E0A040"
  danger-red-dark: "#F07164"
typography:
  figure:
    fontFamily: "Manrope, Segoe UI Variable, sans-serif"
    fontSize: "22px"
    fontWeight: 600
    letterSpacing: "-0.02em"
    fontFeature: "\"tnum\" 1"
  percent:
    fontFamily: "Manrope, Segoe UI Variable, sans-serif"
    fontSize: "15px"
    fontWeight: 600
    letterSpacing: "-0.01em"
    fontFeature: "\"tnum\" 1"
  title:
    fontFamily: "Manrope, Segoe UI Variable, sans-serif"
    fontSize: "13px"
    fontWeight: 600
    letterSpacing: "-0.01em"
  body:
    fontFamily: "Manrope, Segoe UI Variable, sans-serif"
    fontSize: "11.5px"
    fontWeight: 500
    lineHeight: 1.45
  label:
    fontFamily: "Manrope, Segoe UI Variable, sans-serif"
    fontSize: "10.5px"
    fontWeight: 500
  status:
    fontFamily: "Manrope, Segoe UI Variable, sans-serif"
    fontSize: "9.5px"
    fontWeight: 600
    letterSpacing: "0.05em"
  mono:
    fontFamily: "JetBrains Mono, Cascadia Mono, monospace"
    fontSize: "11px"
    fontWeight: 500
rounded:
  window: "10px"
  card: "14px"
  control: "9px"
  pill: "999px"
spacing:
  xs: "4px"
  sm: "8px"
  md: "12px"
  card: "14px"
components:
  sheet-card:
    backgroundColor: "{colors.sheet}"
    textColor: "{colors.ink}"
    rounded: "{rounded.card}"
    padding: "14px"
  ink-block:
    backgroundColor: "{colors.ink-block}"
    textColor: "{colors.ink-block-text}"
    typography: "{typography.figure}"
    rounded: "{rounded.card}"
    padding: "14px"
  usage-bar:
    backgroundColor: "{colors.ink}"
    rounded: "{rounded.pill}"
    height: "4px"
  usage-rail:
    backgroundColor: "{colors.rail}"
    rounded: "{rounded.pill}"
    height: "4px"
  segmented-track:
    backgroundColor: "{colors.rail}"
    rounded: "{rounded.pill}"
    padding: "2px"
  segmented-active:
    backgroundColor: "{colors.sheet}"
    textColor: "{colors.ink}"
    rounded: "{rounded.pill}"
    padding: "3px 10px"
  nav-active:
    backgroundColor: "{colors.ink}"
    textColor: "{colors.ink-block-text}"
    rounded: "{rounded.pill}"
    size: "28px"
  button-primary:
    backgroundColor: "{colors.ink}"
    textColor: "{colors.ink-block-text}"
    rounded: "{rounded.pill}"
    padding: "6px 12px"
  button-quiet:
    backgroundColor: "{colors.sheet}"
    textColor: "{colors.ink}"
    rounded: "{rounded.pill}"
    padding: "6px 12px"
  window-button:
    backgroundColor: "transparent"
    textColor: "{colors.ink-secondary}"
    rounded: "{rounded.control}"
    size: "28px"
---

# Design System: QuotaPulse

## Overview

**Creative North Star: "Tinta sobre papel"**

QuotaPulse reads like a well-set printed report that happens to update itself. The ground is warm paper, cards are crisp white sheets with a hairline edge, and the one figure that matters most right now is printed in a solid ink block. Everything else is black ink at three strengths. Numbers are set in tabular figures and aligned like a ledger; progress is a thin ink line on a soft rail; filters are pills you can feel the edge of.

The register is editorial, clean, tactile and high-contrast: no glass, no glow, no gradient, no tinted blue-grey. Colour is almost absent, so when ochre or red appears on a bar it is unmistakably a warning. Dark mode is the same report printed in chalk on charcoal: neutral, never blue, never neon.

Density is that of a tray widget. Everything must read at 360 px wide; a wider window gains columns, not bigger type.

Confirmed rejections: generic AI-dashboard looks (gradients, glows, frosted glass, oversized diffuse shadows, blue-tinted greys), fake 0 % bars or percentages when no real figure exists, and decoration that carries no information.

**Key Characteristics:**
- Warm paper ground, white sheet cards, 1 px hairlines, almost no shadow.
- One ink block per view for the headline figure.
- Ink is the only colour for data; ochre and red only for threshold warnings, green only for the LIVE dot.
- 4 px progress lines with right-aligned tabular percentages.
- Segmented pills with a high-contrast active state.
- One custom 1.5 px stroke icon set and geometric provider monograms.

## Colors

A three-strength ink on warm paper (light) or chalk on charcoal (dark), with three small status accents.

### Neutral (light)
- **Paper** (`paper` #F4F3EF): the window ground. `paper-soft` (#FAF9F6) for rows and insets inside sheets.
- **Sheet** (`sheet` #FFFFFF): cards, panels, popovers, the active segment of a segmented control.
- **Rail** (`rail` #ECEAE4): progress-bar tracks and segmented-control tracks.
- **Hairline** (`hairline` #E4E1DA): every 1 px border and divider.
- **Ink / Ink Secondary / Ink Muted** (#111111 / #4A4843 / #6F6C64): primary text and data fills; plan labels and descriptions; sub-lines, axis labels and countdown captions. Ink Muted is the lightest text allowed (≥ 4.5:1 on Sheet).

### Neutral (dark)
- **Charcoal** (#141413) ground, **Charcoal Sheet** (#1C1C1A) cards, **Charcoal Raised** (#252523) rows, **Charcoal Rail** (#2E2D2A) tracks and hairlines, **Charcoal Active** (#3A3936) the active chip of a segmented control (it must stand out from the rail).
- **Chalk / Chalk Secondary / Chalk Muted** (#EDEBE6 / #B5B2AA / #8A877F): the three text strengths and the data fill.

### Ink Block
- **Ink Block** (#111111, text #F4F3EF, muted #A9A69E). In dark mode it inverts to Chalk (#EDEBE6) with Charcoal text, so it stays the highest-contrast object on screen in both themes.

### Status
- **Live Green** (`live-green` #23784A / dark #5CC28A): only the LIVE dot and label, and the small "en vivo" chip inside the ink block.
- **Warn Ochre** (`warn-ochre` #A15C00 / dark #E0A040): bar fill at 70–89 %, local/fallback status labels, the left rule of advisory hints.
- **Danger Red** (`danger-red` #B42318 / dark #F07164): bar fill at ≥ 90 %, errors, close-button hover.
- **Estimate Grey** (`estimate-grey` #A8A59D): the fill of an estimated bar (Cursor), always with a `~` before the number.

### Named Rules
**The Ink Is Data Rule.** Below 70 % a bar is plain ink. Colour on a bar means a threshold was crossed and nothing else; providers are never colour-coded.

**The One Block Rule.** At most one ink block per view. It holds the single most important real figure (the live window closest to its limit). With no live figure it holds a plain-language status, never a placeholder number.

**The Estimate Wears Grey Rule.** An approximate figure is shown with a `~` prefix and an Estimate Grey fill, never ink and never a status colour.

## Typography

**Font:** Manrope (bundled, with Segoe UI Variable, sans-serif). **Mono:** JetBrains Mono for paths, commands and identifiers only.

**Character:** a single clean geometric sans set small and tight, with numbers as the only large type, like figures in a printed report.

### Hierarchy
- **Figure** (600, 22 px, −0.02em, tabular): the headline number inside the ink block. The only text above 15 px.
- **Percent** (600, 15 px, tabular): the percentage on each provider card, right-aligned.
- **Title** (600, 13 px): provider names, section titles, page titles.
- **Body** (500, 11.5 px, 1.45): descriptions, notes, hints, settings text.
- **Label** (500, 10.5 px): field labels, metric captions, pills, segmented options.
- **Status** (600, 9.5 px, 0.05em, uppercase): LIVE / ACTIVE / LOCAL labels beside their dot.

### Named Rules
**The Tabular Truth Rule.** Every number that can change (percent, tokens, cost, countdown, reset time) uses `font-variant-numeric: tabular-nums` so nothing jitters on update.

**The Figures Speak Rule.** Size is reserved for numbers. Words never go above 13 px; hierarchy among words comes from weight and ink strength.

## Layout

A single vertical column inside a resizable window (minimum 360 × 420 px). Top to bottom: header bar (brand mark, quick actions, window controls), navigation pills, the ink block, then sheet cards. Window gutter 12 px at 360 px, 16 px from 640 px; card padding 14 px; gap between cards 10 px.

Responsive behaviour adds columns, not size: 1 column below 640 px, 2 from 640 px, 4 from 1320 px. Settings and Compare go to two columns from 900 px.

**360 px rules:** no horizontal overflow, ever (`min-width: 0` on every flex/grid child that holds text). Long labels truncate with an ellipsis and a `title` tooltip, never by clipping mid-glyph. Numbers never wrap or truncate: the label beside them gives way first. Segmented controls that do not fit scroll horizontally inside their own track, never the page.

**Widget mode:** a fixed 360 × 274 window made of a compact ink block (most constrained live figure, no caption row) and one sheet listing a row per provider: monogram, name, 4 px bar, window code (5H, DÍA, MES) and percentage. The same honesty applies: a provider without a live figure shows its status word instead of a bar. Everything fits without scrolling.

**Compare at tray width:** below 480 px the provider gauge is hidden so figures never truncate. Gauges draw no ring at all when there is no real figure.

## Elevation & Depth

Flat by default. Separation comes from the paper/sheet value step and a 1 px hairline. A single faint shadow (`0 1px 2px rgb(17 17 17 / 0.04)`) lifts sheets in light mode only; dark mode uses the hairline alone. No blur, no backdrop-filter, no inner highlights. The OS window stays transparent in `tauri.conf.json`, but the app paints a solid Paper/Charcoal ground across the whole window.

### Named Rules
**The Flat Sheet Rule.** If a surface needs a shadow larger than 2 px to be seen, the value step is wrong; fix the colour, not the shadow.

## Shapes

The window itself is a 10 px sheet (the Windows 11 corner). Cards and panels 14 px radius; controls (inputs, settings rows, window buttons, hint boxes) 9 px; pills, segmented controls, buttons, chips, status dots, bars and rails are fully rounded. Borders are always 1 px.

## Iconography

- **Interface icons:** one custom SVG set on a 16 px grid, 1.5 px stroke, round caps and joins, `currentColor`, no fills except tiny dots. Rendered at 14–16 px. No icon library.
- **Provider monograms:** geometric, single-colour SVG marks on a 20 px grid, drawn in `currentColor` so they follow ink/chalk. They identify the provider by shape alone and are legible at 16 px and 20 px. They are QuotaPulse's own drawings, not the providers' logos.
- **Tray monograms:** the Windows tray badges (`public/providers/*.png`, embedded by `src-tauri/src/tray_badges.rs`) use a simplified variant at 16 px: a chalk glyph on an ink disc, legible on light and dark taskbars. Claude is the six-spoke spark without its ring, ChatGPT is the bare `>_` prompt, and Gemini and Cursor are the same glyphs, enlarged. They are rasterized from SVG with `npx tauri icon <svg> -o <dir> -p 128`.
- **App mark:** an ink rounded square with a chalk gauge: a faint semicircular track, a solid arc for the used part and a pivot dot. Built as SVG (`public/quotapulse-mark.svg`), it is the source for the Tauri icon set.

## Components

### Ink Block (signature)
Solid ink sheet, 14 px radius, 14 px padding. Caption (label, Ink Block Muted), provider monogram + name, the Figure (e.g. `86 %`), the window name and reset time in tabular figures, and a 4 px chalk bar on a translucent rail. With no live figure: the caption plus one sentence (e.g. "Ninguna cuota en vivo ahora · 2 proveedores con datos locales").

### Floating Capsule
A separate borderless, always-on-top window near the top of the screen (top-centre by default, draggable, position remembered). It is an ink pill in both themes, 34 px tall, sized to its content: one 16 px chalk monogram per provider that has a snapshot (plus LiteLLM while its proxy is up), each with a 6 px status dot at its corner (Live Green, Warn Ochre, Danger Red in their light-on-ink variants; ACTIVE hollow). A 2 px bar sits under a monogram only when that provider has a real figure (ink, ochre or red by threshold; Estimate Grey for estimates). A chevron reveals tabular percentages inline. Clicking shows or hides the main window. While it is on, the main window has no taskbar button and the tray is the single app icon.

### Provider Card
White sheet, hairline, 14 px radius, 14 px padding. Row 1: monogram (20 px), name (Title), percent right-aligned (Percent). Row 2: plan label and window name (Body, Ink Secondary). Row 3: 4 px bar. Row 4: status dot + status label on the left, reset countdown right (tabular). No real figure: rows 1, 2 and 4 stay; row 3 becomes a one-line note; no percent is shown.

### Usage Bar
4 px rail (Rail / Charcoal Rail), fully rounded fill: Ink below 70 %, Warn Ochre 70–89 %, Danger Red ≥ 90 %, Estimate Grey for estimates. The percentage sits right-aligned on the same row or above, never inside the bar.

### Segmented Pills
Rail-coloured track with 2 px inset; the active option is a Sheet chip (Charcoal Active in dark) with ink text and a hairline; inactive options are Ink Secondary. Used for periods (7d / 30d / 90d / Personalizado), Gemini source, chart modes and theme.

### Navigation
Icon pills in a row. Active page: solid ink pill with chalk icon (inverted in dark mode). Inactive: Ink Secondary icon, hover shows a Rail background. Provider monograms on the right carry a 6 px health dot.

### Status Label
6 px dot + uppercase Status text. LIVE in Live Green; ACTIVE in Ink with a hollow dot; LOCAL / fallback in Warn Ochre; errors and expired auth in Danger Red.

### Hint Box + Action
Paper Soft background, 2 px left rule in Warn Ochre (Danger Red for errors), 9 px radius, Body text. Fix-it action is a primary ink pill button inside.

### Buttons
- **Primary:** ink pill, chalk text, 600 weight; hover darkens to #000 (dark mode: chalk pill lightens to #fff).
- **Quiet:** sheet pill with hairline border, ink text; hover fills Rail.
- **Window buttons:** 28 px, transparent, Ink Secondary icon; hover fills Rail; close hover fills Danger Red with white glyph.

### Settings Sections & Rows
Each section is a sheet card with an icon + Title header. Rows are Paper Soft insets (9 px) with Body text left and the control right. Toggles are ink pills (on) on a Rail track (off).

## Do's and Don'ts

### Do:
- **Do** keep data in ink; let ochre and red appear only when a threshold is crossed.
- **Do** put the single most important live figure in the one ink block.
- **Do** use tabular figures for every changing number and right-align percentages.
- **Do** design at 360 px first, truncating labels before numbers.
- **Do** draw every icon from the custom 1.5 px set and every provider with its monogram.

### Don't:
- **Don't** show a bar, a 0 % or a computed percentage when no real figure exists; write a plain-language note instead.
- **Don't** use gradients, glows, neon, glass, backdrop blur or shadows larger than 2 px.
- **Don't** use blue-tinted greys anywhere; every neutral is warm (light) or neutral charcoal (dark).
- **Don't** colour-code providers or put more than one ink block in a view.
- **Don't** import an icon library or use the providers' official logos inside the interface.
- **Don't** let any element cause horizontal page scroll at 360 px.
