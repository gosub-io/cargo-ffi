from gosub_engine import EngineConfig, GosubEngine, ZoneConfig, ZoneServices, TabDefaults

print("Hello from Python Gosub!")

# Engine config
cfg = EngineConfig.builder().max_zones(5).build()

# Create engine and start
engine = GosubEngine(cfg)
engine.start()

ev_rx = engine.subscribe_events()

# Zone config
zc = ZoneConfig.builder() \
    .do_not_track(True) \
    .accept_languages("fr-CH, fr;q=0.9, en;q=0.8, de;q=0.7, *;q=0.5") \
    .build()

# Zone services (in-memory, ephemeral cookies)
services = ZoneServices()

# Create zone
zone = engine.create_zone(zc, services)

# Create tab with defaults
defaults = TabDefaults(None, "New Tab", (0, 0, 800, 600))
tab = zone.create_tab(defaults)

# Drive the tab a bit
_ = tab.set_viewport(0, 0, 1024, 768)
_ = tab.navigate("https://news.ycombinator.com")
_ = tab.mouse_move(100.0, 100.0)
_ = tab.mouse_down(100.0, 100.0, "left")
_ = tab.mouse_up(100.0, 100.0, "left")

zone.set_title("My first Zone")
zone.set_description("This is the new description")
zone.set_color(255, 128, 64, 255)

# Simple event pump
for _ in range(50):
    ev = ev_rx.recv()
    if ev is not None:
        print("[event]", ev)

engine.shutdown()
