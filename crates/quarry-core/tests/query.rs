//! M8 queries: compact, paged, under the 4,000-token cap.

use quarry_core::{CommandRegistry, ErrorCode, Source};
use serde_json::{Value, json};

fn run(r: &mut CommandRegistry, name: &str, args: Value) -> Value {
    r.execute(name, args, Source::Agent)
        .unwrap_or_else(|e| panic!("{name}: {}", e.message))
}

#[test]
fn a_100k_triangle_scene_stays_under_the_cap() {
    let mut r = CommandRegistry::new();
    for i in 0..450 {
        run(
            &mut r,
            "mesh.add_primitive",
            json!({ "kind": "sphere", "segments": 16, "position": [i as f32, 0, 0] }),
        );
    }
    let info = run(&mut r, "project.info", json!({}));
    assert!(info["counts"]["triangles"].as_u64().unwrap() >= 100_000);

    let scene = run(&mut r, "query.scene", json!({}));
    assert!(
        scene["tokens"].as_u64().unwrap() < 4000,
        "{}",
        scene["tokens"]
    );
    assert_eq!(scene["more"], true);
    assert_eq!(scene["total"], 450);

    let page = run(
        &mut r,
        "query.mesh",
        json!({ "object": "Sphere", "from": 10, "count": 20 }),
    );
    assert_eq!(
        page["items"].as_array().unwrap().len(),
        20,
        "a page, not the whole mesh"
    );
    assert_eq!(page["items"][0]["i"], 10);
    assert!(page["tokens"].is_u64(), "every query reports its size");

    let err = r
        .execute("query.scene", json!({ "limit": 1000 }), Source::Agent)
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::OutOfBounds);
    assert!(err.message.contains("Narrow it"));

    let o = run(&mut r, "query.object", json!({ "object": "Sphere" }));
    assert_eq!(o["triangles"], 224);
    assert!(o["bounds"].is_array());
}
