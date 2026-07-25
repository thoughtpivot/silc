# inventoryApp

Standalone Silc 0.2.0 inventory application with:

- **Browse** (`/`) — live catalog with category filters
- **Admin** (`/admin`) — create and delete inventory items
- **Assistant** (`/assistant`) — silclm chat grounded on `ui::chat(:context($.items))` with an inventory-assistant `:persona`

## Authored files

- `main.silc`
- `AGENTS.md`
- `.gitignore`

`.runtime/` and `.silc/` are compiler-owned — do not commit or hand-edit them.

## Data model

`InventoryItem`: name, category, location, quantity, reorder_level, notes  
Persisted in SQLite table `inventory_items` through the `Inventory` resource.

## Run

```bash
silc build main.silc
silc main.silc
```

- Web: `http://127.0.0.1:18096/`
- Terminal: `telnet 127.0.0.1 18097`
- API: `http://127.0.0.1:18096/api/inventory_items`

The assistant receives a bounded JSON snapshot of the current inventory as
application context, plus a `:persona` telling it that it is the Inventory
Assistant built on silclm. Ask questions like “which Electronics items are in
stock?”, “what is below reorder level?”, or “who are you?”.
