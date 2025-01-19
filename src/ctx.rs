#[derive(Clone, Debug)]
pub struct Ctx {
	id: String,
	name: String,
	picture: String,
}

// Constructor.
impl Ctx {
	pub fn new(id: String, name: String, picture: String) -> Self {
		Self { 
			id: id,
			name: name,
			picture: picture,
		}
	}
}

// Property Accessors.
impl Ctx {
	pub fn id(&self) -> String {
		self.id.clone()
	}

	pub fn name(&self) -> String {
		self.name.clone()
	}

	pub fn picture(&self) -> String {
		self.picture.clone()
	}
}