# Demo (45 sec)
Disclaimer: the funky green frames are my default terminal color

[demo.webm](https://github.com/user-attachments/assets/2f4d3b79-8c54-427c-9a5e-2bba3ffaa3b3)

# What's This?
A 'workflow engine' that doesn't do any real work.

# Why?
This *was* 'a quick thing to play with ratatui and task primitives'

After a good several hours, it was personal.

# Files
```
└── src
    ├── main.rs        - Tracing, base app rendering (including tui_logger), most logic
    ├── task_picker.rs - Popup modal (wrapped list), static potential task pool, logic to pick task
    ├── tasks.rs       - Enums for status, messages, struct for task data, gross static methods for making tasks
    └── task_table.rs  - 'Main view' (wrapped table) for tracking tasks and their status
```

# Features 
- Lets user spawn *blocking* tasks which sleep and do random accumulation
- Tasks are tracked with struct that keeps their status, flavor text, etc
- Tasks *also* do message passing to communicate their state with host/ui thread
- Lets user request task termination via a broadcast message
- Cool TUI (I think) that displays task status and provides clear controls
- Tracing into file and `tui_logger` widget - latter is abridged to be friendlier to user

# If I Were Doing it Again...
This was written in an 'exploratory style' and error handling, pre-planned architecture, etc. were left out. No regrets there, and I won't try to enumerate everything that _should_ be present on serious software.
- **Use a ratatui template:** async + tui is a huge mess without purposeful structure. Once I tacked on async/multithread, I ended up just making a worse event-based model
- **Use shared structs instead of message passing:** Tokio's MP was cool to try, but I had a tracking struct all along. Obviously, application dependent
- **Start with a clean separation between state and rendering:** almost 'dont care' because I knew this would be <1kLoC and unserious, but it got messy at the end anyway
- **Centralize palette/styles & defer to default colors often:** styling was ad-hoc at the end as I wrapped widgets, which made changes rough. the user is also better at picking non-functional colors they prefer
- **Start with responsive/proportional layouts:** this is almost 'plan ahead', but I hard-coded lengths first and made more flexible constraints later, which was a rough migration
- **Collaborative multiprocessing:** using green threads that awaited regularly seemed more complicated at first, so I went with blocking and made them poll for messages after potentially long delays. That ended up being annoying/slow, and a lot less realistic, anyway
 
