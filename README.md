# Captain's Log

A terminal-based journaling application inspired by the logs and the legacy of decorated _Starfleet_ officers.

Logs are stored in a SQLite database.

## Features

- **Terminal Interface**: Navigate and manage your journal entries directly from the command line.
- **Markdown Support**: Write entries using Markdown for rich text formatting.
- **Export Options**: Export your logs to various formats including ORG, Markdown, or JSON (more are coming).
- **Search Functionality**: Quickly find past entries using keywords or dates.
- **Stardate Mode**: Of course, I can render dates in _pseudo-Stardate_ format for that authentic _Star Trek_ feel.

### Planned Features

- **Encryption**: Secure your journal entries with encryption.
- **Audio Entries**: Record and store audio logs like a true Starfleet officer.
- **Image Support**: Embed images in your journal entries.

## Motivation

I am working on this little application for fun. I would like it to be a **core** journaling application on which I could build different interfaces in the future (a web application, a TUI, and why not, a GUI). For the moment though, I will focus on the command line interface. I got inspired by this model by projects like [Task Warrior](https://taskwarrior.org/) and [jrnl](https://jrnl.sh/).

Why I chouse a SQLite database? After all, we are in an era where "plain text files" are considered a selling point. Well, I like the idea of a single file as a "database" and I consider SQLite an open format anyway. I try my best to make the database as simple and accessible as possible.
