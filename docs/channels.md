# Current channels



## User agent <-> Engine communication 
    (tx, rx) = engine.create_event_channel()

The `tx` will be passed to the `create_zone()`, so a zone can use this to send events back to the UA.
The `rx` part will stay with the UA and will be used to receive events from the engine, zones and all tabs.

## ZoneHandle
The `ZoneHandle` holds a `cmd_tx`. This is a transmit (tx) where you can send `EngineCommands` to the engine. These 
commands will be received (rx) by the engine's run() loop.

 - If the command is an `EngineCommand`, it will control things on the engine (shutdown, some runtime settings)
 - If the command is a `ZoneCommand`, it will control zone things (create/close zone, list zones, set zone properties)
 - If the command is a `TabCommand`, it will be ignored (maybe give a `EngineError::IncorrectCommand` or something)

The `ZoneHandle` itself holds some functions to make it easier to communicate with the engine:

```rust
impl ZoneHandle {
    pub async fn set_color(&self, color: [u8; 4]) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx.send(EngineCommand::Zone(ZoneCommand::SetColor {
            zone_id: self.zone_id,
            color,
            reply: tx,
        })).await?;
        rx.await?
    }
}
```

Questions:
- Why are we sending engine commands to a "ZoneHandle"? Shouldn't we do this on an "EngineHandle"?


## TabHandle
The `TabHandle` holds a `tab_cmd_tx`. This is a transmit (tx) where you can send `TabCommands` to the tab. These
commands will be received (rx) by the tab's own thread/task that is running in the background.
    




```mermaid
sequenceDiagram
    autonumber
    actor UA
    participant Eng as "Engine (cmd_rx)"
    participant Zone as "Zone task"
    participant Tab as "Tab task"

    %% Note over UA,Eng: "Event channel: (tx, rx) = engine.create_event_channel()<br/>UA keeps rx (to receive EngineEvent); Engine/Zone/Tab clone tx (to send)"

    UA->>Eng: "EngineHandle.create_zone(cfg)<br>(send EngineCommand::CreateZone{reply})"
    Eng-->>UA: "ZoneHandle(zone_id, engine_tx)"
    
    UA->>Eng: "ZoneHandle.create_tab(url)<br>(send EngineCommand::Zone(CreateTab{zone_id, reply}))"
    Eng->>Zone: "create_tab(zone_id, url)"
    Zone->>Eng: "new Tab created -> (tab_tx, tab_rx)"
    Eng-->>UA: "TabHandle(tab_id, tab_tx)"
    
    Note over UA,Tab: "Tab channel is direct: TabHandle holds tab_tx (Sender<TabCommand>)"
    
    UA->>Tab: "TabHandle.navigate(\"https://example\")<br>(send TabCommand::Navigate)"
    Tab-->>Eng: "EngineEvent::NavigationStarted(tab_id)"
    Eng-->>UA: "EngineEvent via event_tx (UA's rx receives)"
    
    UA->>Tab: "TabHandle.set_viewport(viewport)<br>(send TabCommand::SetViewport)"
    Tab-->>Eng: "EngineEvent::FrameReady(handle)"
    Eng-->>UA: "EngineEvent via event_tx"
    
    %% Note over UA,Eng: "EngineHandle still exposes engine-wide ops (Shutdown, switch backend, etc.)<br/>ZoneHandle only exposes zone-scoped methods; TabHandle only tab-scoped methods"

```


```mermaid

graph TD
    subgraph UA["User Agent"]
        UA_EventRx["event_rx (Receiver<EngineEvent>)"]
        UA_EngineHandle["EngineHandle"]
        UA_ZoneHandle["ZoneHandle"]
        UA_TabHandle["TabHandle"]
    end

    subgraph Eng["Engine"]
        Eng_EventTx["event_tx (Sender<EngineEvent>)"]
        Eng_CmdRx["cmd_rx (Receiver<EngineCommand>)"]
        Eng_CmdTx["cmd_tx (Sender<EngineCommand>)"]
    end

    subgraph Zone["Zone"]
        Zone_Task["Zone Task"]
        Zone_Id["zone_id"]
    end

    subgraph Tab["Tab"]
        Tab_Task["Tab Task"]
        Tab_CmdRx["tab_cmd_rx (Receiver<TabCommand>)"]
        Tab_CmdTx["tab_cmd_tx (Sender<TabCommand>)"]
    end

    %% Event channel
    Eng_EventTx --> UA_EventRx

    %% Engine command channel
    UA_EngineHandle --> Eng_CmdRx
    UA_ZoneHandle --> Eng_CmdRx

    %% Zone creates Tab
    Eng_CmdRx --> Zone_Task
    Zone_Task --> Tab_CmdRx
    UA_TabHandle --> Tab_CmdRx

    %% Tab sends events back through Engine to UA
    Tab_Task --> Eng_EventTx

```