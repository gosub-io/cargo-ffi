```mermaid
graph TD

    subgraph engine["Gosub Engine"]
        Engine["Engine"] --> ZoneManager["ZoneManager"]
        ProcessRenderer["Global Renderer"]
    end
    
    subgraph za["Zone A"]
        ZoneManager --> ZoneA["Zone A"]
        CookieStoreA["CookieStore A"] --> ZoneA
        LocalStorageA["LocalStorage A"] --> ZoneA
        SessionStorageA["SessionStorage A"] --> ZoneA
        ZoneA --> Tab1["Tab 1"]
        ZoneA --> Tab2["Tab 2"]
    end

    subgraph zb["Zone B"]
        ZoneManager --> ZoneB["Zone B"]
        CookieStoreB["CookieStore B"] --> ZoneB
        LocalStorageB["LocalStorage B"] --> ZoneB
        SessionStorageB["SessionStorage B"] --> ZoneB
    end
    
    subgraph tab1
        Tab1 --> EngineInstance1["EngineInstance 1"]
        EngineInstance1 --> Threads1
        EngineInstance1 --> ProcessRenderer
    end
    
    subgraph tab2
        Tab2 --> EngineInstance2["EngineInstance 2"]
        EngineInstance2 --> ProcessRenderer
        EngineInstance2 --> Threads2
    end
    
```