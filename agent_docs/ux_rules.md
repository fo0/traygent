# Owner UX rules (binding)

Carried over from RepoWatch, where the owner set them (#18 there), and adapted to Traygent's domain. They override every older UI statement in `README.md`, `docs/`, `docs/adr/` and `agent_docs/`. Read them before any UI change; the UI review checks each one (`agent_docs/review_process.md → UI Review`).

1. **Compact first.** Single-line rows over cards. Smallest readable size. No decorative headings, badges or step lists. The „Neue Session“ composer and the „Zuletzt“ list card (RepoWatch's list card, owner request #17) are the only cards.
2. **Minimal text.** Labels, not sentences. At most one short help line per form. No explanatory paragraphs. Details go into tooltips. Never describe what the UI obviously does.
3. **Identify items by what differs.** In „Zuletzt“ the folder name is primary, its parent path secondary; agent, environment and options follow as icons and chips.
4. **No manual configuration the app can infer.** Installed agent CLIs, their versions and Docker are detected, not entered; recent folders come from the sessions started. Options start from the agent's saved defaults.
5. **Settings are staged.** Changes apply with one primary „Speichern“ next to „Abbrechen“. Immediate actions are explicit buttons.
6. **One primary action per form or flow, bottom right.** „Starten“ in the composer; a missing CLI turns it into „Installieren …“, never a dead start. A completed flow closes or returns by itself — the user never has to press „Schließen“ after success.
7. **Feedback goes away.** Success notices close after 3 s (an undo offer stays 8 s). Only warnings and errors persist. Nothing may get stuck on screen.
8. **Toggles stay in place.** A toggle changes its state at the same spot; no separate back button. The brand at the top left leads to the dashboard.
9. **Updates install in the app.** „Nach Updates suchen“ → „Update verfügbar“ with at most 5 changelog bullets → „Installieren“ → progress bar → the app installs and restarts itself. Never send the user to a web page to update (`docs/RELEASES.md`).
10. **Honest evidence.** Screenshots or previews shown to the owner come from the exact build being delivered. Never present a mock-up as the result.
11. **Owner requests beat older docs.** When a doc or ADR contradicts an owner request, supersede the doc or ADR in the same change instead of following it.

**Accessibility adds no visible prose.** Accessible names and tooltips may carry detail; visible text never grows to satisfy accessibility. An icon button gets an accessible name, not a caption.
