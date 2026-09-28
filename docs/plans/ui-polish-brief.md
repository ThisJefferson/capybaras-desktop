# Capybaras — UI polish brief

**Source:** Jeff, 2026-09-28. **Scope: the graphical interface only.**

> I want you to focus on just that graphical UI.

**Hard boundary.** Visual identity, layout, typography, colour, spacing, components, responsiveness, animation, interaction states, perceived performance. **Do not redesign business logic, agent behaviour, backend, permissions, data models, API calls, or security decision-making.** Preserve existing functionality unless a visual change requires a small frontend-only adjustment.

**Goal.** Capybaras should feel like a premium, friendly, calm, highly intelligent decision-support product. Smooth and satisfying — like thoughtfully crafted software, not a generic dashboard.

**High-level inspiration, no copying:** the kinetic, music-like responsiveness of classic software such as Sonique; the warm clarity and frictionless simplicity associated with TunnelBear; modern premium productivity and observability tools. **Do not copy any product's logo, layouts, illustrations, assets, wording, colours, icons or trade dress.** The Capybaras identity must be original.

---

## 1. Visual personality

Should feel: calm, safe, steady, trustworthy · friendly and warm, never cold or militaristic · smart and modern without looking like generic AI SaaS · protective without hacker aesthetics · rich and expressive without clutter or childishness · smooth, tactile and alive without wasting motion.

The metaphor: **a calm guide watching over complex systems** — relaxed, observant, grounded, capable, reassuring.

**Avoid:** matrix-green text · neon hacker themes · excessive locks, shields, skulls, warning stripes, terminal decoration · generic purple-gradient AI dashboards · overuse of floating rounded cards · excessive glassmorphism, blur, gradients, shadows · tiny text, weak contrast, cramped controls, noisy data tables.

## 2. Audit first

Before changing code: inspect every existing screen, navigation pattern, modal, form, table, empty/loading/error state. Identify inconsistent styling, weak hierarchy, unclear primary actions, poor spacing, visually heavy areas, friction in common flows. Identify performance problems — layout shifts, janky animation, delayed feedback, oversized assets, excessive rendering. **Produce a brief visual upgrade plan, then implement it.**

## 3. Design system

**Colour.** Warm, calm base: deep ink, charcoal, muted navy, warm stone, soft grey, subtle earth tones. An original signature accent (moss green, eucalyptus, teal, river blue, soft coral or sunset amber). Green/teal for healthy or verified. Amber/gold for attention, pending review, caution. **Red sparingly, only for meaningful danger.** Neutral greys for informational and inactive. Accessible contrast in every theme and state. **Never rely on colour alone** — always with labels, icons or shape.

**Typography.** A strong display style for product-level moments and major headings; a clean legible sans for UI text, body, controls, data. A disciplined type scale covering page titles, section headings, labels, body, helper text, code/data, captions, status labels. **Establish hierarchy with weight, spacing and colour before adding decoration.** Comfortable line lengths and paragraph spacing.

**Spacing and layout.** One consistent spacing scale, applied everywhere. Room to breathe for important content. Screen structure immediately understandable: navigation, page context, primary workspace, supporting information, primary action. Consistent container widths, grid behaviour, side-panel widths, table density, breakpoints. Avoid both over-empty pages and overcrowded dashboards. Use alignment aggressively — every component intentionally placed.

**Surface and depth.** Restrained layering. Prefer subtle surface contrast, borders, tonal elevation and carefully controlled shadows. Translucency or blur only where it aids navigation or overlays. **Avoid card soup** — cards only where grouping or interaction truly benefits. The primary workspace should feel open and focused.

## 4. Components

**Navigation** — current location obvious; primary nav scannable; responsive behaviour intentional (sidebar, compact rail, top nav, mobile drawer); subtle active-state motion and a clear visual anchor; stable during transitions.

**Buttons** — primary, secondary, tertiary, destructive, icon-only, loading, disabled, split. The primary action unmistakable without every button being loud. Comfortable target sizes for mouse and touch. Refined hover, pressed, focus, loading, success, disabled. Labels direct and action-oriented.

**Inputs and forms** — clear labels, helpful supporting text, obvious focus states, inline validation, error recovery. Lightweight and guided rather than administrative. Sensible grouping and progressive disclosure. **Never rely on placeholder text as a label.**

**Status and risk indicators** — an elegant, consistent family for safe, informational, pending, caution, warning, high-priority and critical. Colour + iconography + labels + shape. Chips, severity tags, activity indicators, progress indicators, timelines and alerts designed as one family. **Critical information easy to spot without turning the whole interface red.**

**Data-heavy views** — tables, activity feeds, reports, logs, findings, dashboards easy to scan. Density controls where appropriate. Emphasise important columns, de-emphasise secondary metadata. Sticky headers, thoughtful row hover and selection states, expandable detail panels, readable empty states. Prevent monotony with grouping, hierarchy and purposeful whitespace. **Charts only where they clarify a decision or trend.**

**Modals, drawers, overlays** — for focused tasks, confirmations and detailed inspection, never as a substitute for page structure. Animate with clear spatial continuity. Preserve context behind the overlay without visual busyness. Escape-key behaviour, focus management, accessible labels.

**Empty, loading and error states** — every state designed deliberately. Empty states explain the page's purpose and give one clear next action. Loading uses layout-matching **skeletons** rather than generic spinners wherever possible. Long-running activity shows meaningful stages. Errors are calm, specific, readable, constructive. **Never show a raw technical error as the primary user-facing experience.**

## 5. Motion

Motion must communicate: input acknowledgement, navigation continuity, hierarchy changes, loading and progress, success and completion, selection and focus, state changes.

**Rules.** Immediate visual feedback for every click, tap, drag, keyboard command and form submission. Spring-like easing that feels natural and high quality. **≈120–200 ms** small feedback · **180–320 ms** component transitions · **250–450 ms** larger panel/page transitions. Prefer **transform and opacity**; avoid animating layout properties where transform can do the job. No slow, bouncy, distracting, looping or attention-seeking motion. Do not animate decorative elements if it costs readability, battery or responsiveness. Subtle page transitions only where they preserve orientation. Microinteractions for hover, press, selection, toggles, validation, completion. **Respect `prefers-reduced-motion`** — reduced motion must still preserve hierarchy and feedback via opacity, colour and instant state change. 60 fps on normal modern devices.

**Signature motion language:** a gentle "settling" spring when panels, controls and results appear · quiet controlled pulses only for live activity or pending work · small satisfying confirmation movement when something is resolved, saved or verified · smooth morphing or sliding state changes for status chips and progress indicators · ambient motion, if any, nearly imperceptible and limited to non-essential backgrounds.

## 6. Responsive

**Desktop** — spacious workspace, visible context, optional persistent navigation, high density without clutter. **Tablet** — adaptive multi-column, collapsible panels, touch-friendly. **Mobile** — focused single-column flows, bottom sheets or drawers, clear sticky primary actions, large touch targets, no cramped data tables. **Do not merely shrink the desktop layout** — reconsider hierarchy and interaction at each breakpoint.

## 7. Accessibility

Semantic HTML and accessible names. Full keyboard navigation. Strong visible focus states. Accessible text and interface contrast. Never meaning via colour alone. Comfortable touch targets. Support zoom, screen readers, reduced motion, responsive text sizing. No hover-only interactions. Error, warning and status messages understandable without visual cues.

## 8. Performance and polish

Eliminate layout shifts. Avoid oversized images, heavy video backgrounds, excessive blur, deep nested shadows, huge animation libraries. Optimise fonts, avoid unnecessary weights. Lazy-load non-critical visuals and routes. Keep scrolling smooth. Avoid unnecessary rerenders. Skeletons and optimistic feedback where appropriate. **The first meaningful view should be useful before decorative assets finish loading.** Test on slower hardware and narrow screens.

## 9. Implementation requirements

Make the changes in the actual codebase. Reuse or improve the current component architecture. Create reusable design tokens for colour, spacing, typography, radii, elevation, breakpoints and motion. Preserve existing functionality. No backend or agent-logic changes unless strictly necessary for a UI state that already exists. Clean, maintainable, responsive, accessible. **No large dependencies merely for decorative effects.** Use existing project conventions.

## 10. Deliverables at the end

1. **Visual-system summary** — palette and semantic status colours; type scale; spacing scale; radii and elevation rules; button/input/navigation/status conventions; motion durations and easing.
2. **Screen-by-screen summary** — what was improved, what was simplified, what motion was added, how mobile was handled.
3. **Performance summary** — what was optimised; what effects were deliberately avoided or limited.
4. **Remaining-polish list** — the highest-impact UI work to do next.

## 11. Quality bar

The finished UI should communicate: *"This is calm and under control." · "Important information is easy to understand." · "I always know what I can do next." · "This feels fast, alive, and beautifully made." · "This security product helps me think clearly rather than making me anxious."*

**Prioritise:** clarity over decoration · warmth over coldness · strong hierarchy over excessive cards · purposeful motion over visual noise · smooth performance over expensive effects.

---

## Project constraints this brief must respect (added by the implementer, not Jeff)

- **`BRAND.md`** already defines the identity: the name, the herd, the four states, the Rio de Janeiro visual language, a copy deck. **Extend it; do not replace it.** Same for `docs/plans/M4-visual-design.md`.
- **Design tokens are generated and checked.** `npm run build:tokens` writes `tokens.css`; `scripts/check-tokens.mjs` validates every reference and property. A new token must be declared *and* referenced, or the check fails.
- **CSP is `script-src 'self'`.** The frontend is external files deliberately; no inline scripts or inline event handlers.
- **`textContent` only** for anything derived from remote data (threat-model T3). The model catalogue is remote data rendered in a trusted position — do not introduce `innerHTML` for it.
- **Keep the gate green.** `npm run verify` must exit 0; `verify:ui`, `verify:acceptance` and `verify:onboarding` render the real frontend and assert against it, so visual changes are covered by those checks.
- **The four herd states are real**, driven by sidecar state. No animation may be decorative.
