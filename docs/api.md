UA:

    initializes the framework GUI stuff
    instantiates the gobus engine
    
    creates a zone (UA is the owner)
    creates a tab in the zone (UA is the owner)

    tab has tx channel
    tab has rx channel

    tab.tx -> navigate to https://gosub.io
            tab -> browsercontext -> loads stuff... yadayayayd

    tab.tx -> start drawables

    UI event loop:

        tabs = [ tab1, tab2, tab3 ]        

        tab.rx <- making connection to page
        tab rx <- handshake TLS
        .....
        tab.rx <- receives page loaded event?

        tab.rx <- drawable epoch 1
        tab.rx <- drawable epoch 2
        tab.rx <- drawable epoch 3
        tab.rx <- drawable epoch 4   16ms
        tab.rx <- drawable epoch 5   16ms
        ...
        tab.rx <- drawable epoch n

        <5s wait>

        tab.tx -> mouse move events

        tab.rx <- drawable epoch 6
        tab.rx <- drawable epoch 7

        tab.tx -> stop drawing

        tab.tx -> mouse move events


let engine = GoubEngine::new();

let zone = engine->create_zone();

let rx = useragent.window.rx_channel();
let rx = usagagent.global.rx_channel();
let rx = channel();
let tab = zone->tab_builder().with_channel(rx).build();

struct Tab {
    tx: channel
}

let tab_list = Vec<Tab>

tab_list[0].emit(NavigateCommand { url: "https://gosub.io" })
tab_list[0].emit(ResumeDrawing { fps: 10 } )
tab_list[0].emit(SuspendDrawing)
tab_list[0].emit(MouseMoveEvent { x: 0.0, y: 0.0 } )

event_loop:
    if event = rx.recv() {
        match event {
            RedrawEvent { surface: ExternalSurface } => {
                .. paint it 
            ConnectionEvent { tabid, url: ... } => {
                println("Connection has been made to {}", url);
            }
        }
    }
