# QuickShards

This is a personal executable to quickly append logs to my Obsidian daily note. However, because nothing is really Obsidian-specific, I guess it can be used to append timestamped entries to any file.

## Usage

- Install with

```bash
cargo instal --git git@github.com:THeK3nger/quickshards.git
```

- Create a new configuration file with

```bash
qs -e
```

- Add your configuration. Use this as an example

```toml
obsidian_vault_path = "/Users/<your_user>/Path/To/Vault"
daily_path = "Daily Notes"
working_memory_file_path = "__WM.md"
```

- Add an entry with

```bash
qs "Whatever I want to add to my note."
```

_(there is not a lot of customization because this is tailored to my notes, but I am open to PRs if you want)_.
