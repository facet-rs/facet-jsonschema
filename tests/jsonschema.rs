extern crate alloc;

use alloc::{rc::Rc, sync::Arc};

use facet::Facet;
use facet_jsonschema::to_string;
use insta::assert_snapshot;

#[test]
fn basic() {
    /// Test documentation
    #[derive(Facet)]
    #[facet(facet_jsonschema::id = "http://example.com/schema")]
    struct TestStruct {
        /// Test doc1
        string_field: String,
        /// Test doc2
        int_field: u32,
        vec_field: Vec<bool>,
        slice_field: &'static [f64],
        array_field: [f64; 3],
    }

    let schema = to_string::<TestStruct>();
    assert_snapshot!("basic", schema);
}

#[test]
fn pointers() {
    /// Test documentation
    #[derive(Facet)]
    #[facet(facet_jsonschema::id = "http://example.com/schema")]
    struct TestStruct<'a> {
        normal_pointer: &'a str,
        box_pointer: Box<u32>,
        arc: Arc<u32>,
        rc: Rc<u32>,
        #[allow(clippy::redundant_allocation)]
        nested: Rc<&'a Arc<&'a *const u32>>,
    }

    let schema = to_string::<TestStruct>();
    assert_snapshot!("pointers", schema);
}
