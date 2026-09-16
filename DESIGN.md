# Flow desktop UI

## Reference research

The redesign uses desktop application patterns rather than a marketing-page layout.

- GNOME Human Interface Guidelines, [Windows](https://developer.gnome.org/hig/patterns/containers/windows.html): size the window for its controls and avoid excessive empty space. Applied to compact setup groups and a persistent save footer.
- GNOME HIG, [Navigation](https://developer.gnome.org/hig/guidelines/navigation.html): focused views and simple flat navigation. Applied to the Settings/History view switcher in a single window.
- [shadcn sidebar component examples](https://ui.shadcn.com/docs/components/base/sidebar) and [sidebar blocks](https://ui.shadcn.com/blocks/sidebar): reference for neutral surfaces, restrained borders, active navigation, and separated content areas. Used as visual references, not copied templates or a React dependency. Flow has only two views, so it uses a view switcher rather than a permanent sidebar.

## Implementation decisions

- Settings leads with editable controls, not a hero, photograph, carousel, or promotional copy.
- Two settings columns at desktop widths; one at narrow widths. Transcription fields are grouped separately from switches and shortcut instructions.
- Neutral background, distinct input/panel surfaces, and a single purple accent matching Flow's existing identity.
- System light/dark styles, keyboard focus indicators, reduced motion, safe transcript rendering, and confirmation before clearing history.
- Existing Tauri commands and configuration fields are preserved. The browser preview cannot save to desktop settings.
- Audio upload and text-history handling are described where they affect user choices.

## Validation

Browser interaction checks use a mocked Tauri bridge with sample data. They cover configuration loading/saving/error handling, API-key reveal, status events, history search/deletion/confirmation, and responsive layouts. These do not replace live microphone, Groq, or paste-back testing in the desktop app.
