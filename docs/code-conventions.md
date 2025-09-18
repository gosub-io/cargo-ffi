## Code conventions

Maybe.


### Grouping convention in structs

Suggested grouping convention

1. **Identifiers**
  - Always list IDs and handles that uniquely identify the struct instance first.
  - Example: `tab_id`, `zone_id`, `thread_id`.
2. **Context / ownership / state**
  - Things that give the struct access to shared context, state, or configuration.
  - Example: `zone_context`, `runtime_config`, `engine_ref`.
3. **Services / capabilities**
  - Fields representing injected services or capability sets.
  - Example: `services`, `effective_services`, `cookie_jar`.
4. **Communication channels**
  - Sinks, senders, event buses, or anything that points upwards or outwards.
  - Example: `sink`, `tx`, `reply_chan`.
5. **Other internals**
  - Scratch buffers, caches, internal flags.

Example: pending_tasks, inflight_requests, dirty.x


```rust

pub struct TabWorker {
    // 1. Identifiers
    pub tab_id: TabId,
    pub zone_id: ZoneId,

    // 2. Context / ownership
    zone_context: Arc<ZoneContext>,

    // 3. Services / capabilities
    services: EffectiveTabServices,

    // 4. Communication channels
    sink: Arc<TabSink>,
}

```

