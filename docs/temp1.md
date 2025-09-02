```

pub struct GosubEngine:
	pub fn new()					// Create new instance
	pub fn create_event_channel()			// Create new event tx/rx to communicate with UI
	pub fn engine_handle()				// Return engine handle
	pub fn set_backend_renderer()			// Update backend renderer
	pub fn command_sender()				// cfg(test): Returns the engine command sender
	pub fn create_zone()				// Create a new zone and return zone handle
	pub async fn run()				// "Run" the engine in a new thread/task
	
	async fn handle_zone_command()			// Handle zone commands found in the command receive channel
	fn zone_by_id()					// Return zone by id
	async fn create_tab_in_zone()			// Create a new tab in the zone


pub struct ZoneHandle
	pub fn new()					// Create a new zone handle
	pub fn id()					// Return zone id
	pub async fn set_title()			// Set title of the zone
	pub async fn set_icon()				// Set icon of the zone
	pub async fn set_description()			// Set descirption of the zone
	pub async fn set_color()			// Set color of the zone	
	pub async fn create_tab()			// Create a tab in the zone
	pub async fn close_tab()			// Close tab in the zone
	pub async fn list_tabs()			// List all tabs in the zone
	// pub async fn tab_title()			// Change tab title (not needed here?)


pub struct Zone
	pub fn new_with_id()				// Create new zone with specific ID
	pub fn new()					// Create new zone
	pub fn set_title()				// Set title of zone
	pub fn set_icon()				// Set icon of zone
	pub fn set_description()			// Set description of zone
	pub fn set_color()				// Set color of zone
	pub fn services()				// Return services
	pub async fn create_tab()			// Create new tab in zone
	pub fn local_area()				// Get local storage
	pub fn session_area()				// Get session storage
	fn spawn_storage_forward()			// Spawn things... not sure anymore
	pub fn close_tab()				// Close tab
	pub fn list_tabs()				// List tab
	
	
pub struct TabHandle
	pub fn id()					// Returns id of the tab
	pub fn engine_tx()				// Return engine send channel
	

pub struct Tab
	pub fn new()					// Create a new tab and return tab handle
	pub fn navigate_to()				// Navigate tab to url
	pub fn bind_storage()				// Bind local/session storage to tab
	pub fn set_viewport()				// Set the viewport of the tab
	pub fn thumbnail()				// Return the thumbnail of the tab (if any)	
	pub(crate) fn dispatch_storage_events()		// Dispatch local/session storage events to other docs 
	fn ensure_surface()				// Ensure we have a correct (and large enough) render surface
	pub fn handle_event()				// Handle any incoming events
	pub fn execute_command()			// Execute any commands
	

```