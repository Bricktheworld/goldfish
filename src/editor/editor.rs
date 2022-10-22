use goldfish::GoldfishEngine;

fn main()
{
	let engine = GoldfishEngine::new("Goldfish Editor");
	engine.run(move |_| {
		// println!("Editor update!");
	});
}
