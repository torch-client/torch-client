use std::rc::Rc;

use azalea_brigadier::{builder::argument_builder::ArgumentBuilder, prelude::*};

#[test]
fn test_arguments() {
    let builder: ArgumentBuilder<()> = literal("foo");

    let argument: ArgumentBuilder<()> = argument("bar", integer());
    let builder = builder.then(argument.clone());
    assert_eq!(builder.arguments().children.len(), 1);
    let built_argument = Rc::new(argument.build());
    assert!(
        builder
            .arguments()
            .children
            .values()
            .any(|e| *e.read() == *built_argument)
    );
}
