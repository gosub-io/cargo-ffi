pub trait Storable {
    fn save(&self) -> anyhow::Result<()>;
    fn load(&mut self) -> anyhow::Result<()>;
}

pub struct Storage {
}

impl Storable for Storage {
    fn save(&self) -> anyhow::Result<()> {
        // Implement saving logic here
        Ok(())
    }

    fn load(&mut self) -> anyhow::Result<()> {
        // Implement loading logic here
        Ok(())
    }
}

impl Storage {
    pub(crate) fn new() -> Storage {
        Storage {
        }
    }
}