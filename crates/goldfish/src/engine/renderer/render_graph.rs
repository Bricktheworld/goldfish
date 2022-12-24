pub struct RecordedPass {
	pub name: &'static str,
	pub index: usize,
}

pub struct RenderGraph {
	pub passes: Vec<RecordedPass>,
}

impl RenderGraph {
	pub fn new() -> Self {
		Self {
			passes: Default::default(),
		}
	}

	pub fn add_pass<'a>(&'a mut self, name: &'static str) -> PassBuilder<'a> {
		let index = self.passes.len();
		let recorded = Some(RecordedPass { name, index });

		PassBuilder {
			graph: self,
			index,
			recorded,
		}
	}

	fn record_pass(&mut self, pass: RecordedPass) {
		self.passes.push(pass);
	}
}

pub struct PassBuilder<'a> {
	pub graph: &'a mut RenderGraph,
	pub index: usize,
	pub recorded: Option<RecordedPass>,
}

impl<'a> Drop for PassBuilder<'a> {
	fn drop(&mut self) {
		self.graph.record_pass(self.recorded.take().unwrap());
	}
}
