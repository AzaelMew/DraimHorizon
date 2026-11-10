# UI organization and review

## Navigation

- **Waypoints**: saved places, world filter, create/edit and list deletion confirmation.
- **Map and claims**: Claims (list, filters, refresh), Map, Borders, Colors and Storage. The explored/seed maps' Settings button opens Map.
- **Friends**: friends, online mod users, invitations and important players. Identity verification and consent are unchanged.
- **Chat**: Style / position (including the translation switches), Notifications, Anti-spam and DeepL.
- **Trading**: a dedicated navigation destination. General, economy overlay, updates, profit and fees preferences are grouped here; **Open trading workspace** opens prices, auction observations, estimates and sale planning. Trading is not duplicated as a normal Settings category.
- **Experiments**: ore overlay, automation tasks and helpers. Explicit experimental consent is preserved; opening a page does not start a task.
- **Settings**: General, Network, Player list and Sidebar. Existing persistence keys and defaults are unchanged.

Wide, sufficiently tall GUIs use the labelled rail. Smaller GUIs use a labelled Section selector with a two-column destination chooser, rather than six truncated tab captions. Settings and language remain available outside the chooser.

## Shared controls

- `Name − value` / `Name + value` pairs render as one stepper row: name, value cell, − and + buttons.
- `Name: value` choices show the value on the right with a chevron; `Name: on/off` (and yes/no) rows show a switch and are never drawn as selected buttons.
- Text inputs are inset with a muted label; arrow paging buttons use Lucide chevrons; paging is hidden when there is one page.

## Secondary-screen review

| Screen or menu | Organization and interaction |
| --- | --- |
| Trading workspace overview | Summary cards and explicit actions; one Back path returns to Trading preferences. |
| Prices, auction, profit and selling-plan tabs | Large GUIs retain list/detail columns. Small GUIs show the list or full-width details with Back to list. Details scroll with visible controls, wheel over the details, or Page Up/Down when no text field owns input. A compact detail view consumes Escape before leaving the workspace. |
| Trading sort chooser | Two columns on short/narrow GUIs keep all seven choices visible. |
| Auction catalogue | Opening the auction walks All listings in the background and shows every lot in one searchable, sortable list with one pager. Buying confirms (switchable: Ask before buying at the auction) and sends `/ah buy <number>`, which buys at once; My listings, Mail and server confirmation pages show the server's own page. Market keeps one pager across server pages. Compact by default; Trading → General → Large auction and market windows for GUI scale 2. |
| Auction and Market inventory replacements | Existing server-supplied categories, search, item cards and explicit transaction gates remain. Confirmation dialogs use the shared modal frame and action-specific Buy/Sell/Place bid/Claim labels. |
| Explored map | Dimension navigation and responsive map tools are preserved. Grid state is named; Fit claims explains the bounds action; mob filters have a visible close control. |
| Seed map | Shared dimension/tool geometry; structure/biome filter sections, search and existing found markers remain. Claims overlay state is named. |
| Waypoint list | World filtering, create-here action, edit-on-row and pagination. Deleting from the list requires a named confirmation dialog. Existing deliberate double-click map gestures remain unchanged. |
| Waypoint editor | Persistent Name, Type, Color and X/Y/Z labels, palette swatches, named world, Save/Cancel. Existing compact geometry is preserved. |
| Inventory tools | Sorting, Buttons and Safety categories. Each movement direction has its own visible control; right-click reversal remains an optional shortcut. Template, reset, exclusions and virtual-menu protection remain available. Save/Cancel stay fixed while settings paginate. |
| Translation language editor | Centred card: My messages (I write in → Send as) and Chat messages (Chat is in → Show as), all languages at once with a check on the choice, common-language switch, Cancel/Save. Paging only when the GUI is too small. |
| Appearance | Full-size control rows and fixed Save/Cancel. Small GUIs paginate instead of shrinking rows; keyboard focus reveals the appropriate page. |
| Helpers | Full-height toggles and explicit click-button/rate controls. Small GUIs paginate while larger GUIs retain explanatory text. |
| Remaining automation forms | Existing task choices, preview/start/stop controls, material requirements and server acknowledgement gates are retained. Shared control styling and tooltips apply; no gameplay driver was rewritten for this review. |
| Friend, claim and chat settings | Existing filters, explicit player actions, editable colors, provider fields and per-section paging are retained. Secrets stay masked and are not disclosed by hover hints. |

Russian is the default; interface labels and new explanations have Russian and English counterparts. Player names, message bodies, item content, coordinates and user-entered values are not translated by presentation changes.

## Verification boundaries

`tests/ui_navigation.rs` exercises actual menu clicks, compact navigation, all Trading setting keys, helper reachability, translation-language choices, modal/editor geometry, responsive trading geometry and bounded detail scrolling. Existing tests cover preferences, saved data and gameplay logic. Native-client checking compiles the adapters after applying the official pinned Horizon mixins.

`cargo run --locked --example ui_preview` generates **offline layout previews**, not in-game screenshots. Previews use production menu geometry, labels, palette and official Lucide paths, but use system fonts and simplified compositing. They do not prove live inventory/transaction rendering or SMP behavior. Those flows require a live game session; neither an offline preview nor a successful compile should be reported as live visual verification.
