// SPDX-License-Identifier: GPL-3.0-or-later
//! M98: semantic value writes resolve public scalar owners without using lexical aliases.
use geosolve_sketch_code::{
    CompiledManagedSource, ManagedValue, SemanticOutputPath, SemanticSymbol, UnitLiteral,
    derive_managed_value_mutation,
};

#[test]
fn semantic_value_preparation_resolves_parameter_symbols_and_ordinary_scalar_bindings() {
    for (fixture, symbol, alias, prior) in [
        (
            include_str!("fixtures/m97-source-metadata.json"),
            "width",
            Some("x"),
            12.0,
        ),
        (
            include_str!(
                "../../../packages/geosolve-sketch-code/test/fixtures/managed-compiler-envelope.json"
            ),
            "sharedRadius",
            None,
            4.0,
        ),
    ] {
        let current = CompiledManagedSource::from_json(fixture).unwrap();
        let before = current.clone();
        let replacement = ManagedValue::Unit(UnitLiteral {
            unit: "mm".into(),
            value: 11.0,
        });
        let mutation = derive_managed_value_mutation(
            &current,
            &SemanticSymbol(symbol.into()),
            &SemanticOutputPath::default(),
            replacement.clone(),
        )
        .expect("public scalar owner must be writable");
        assert_eq!(mutation.declaration, symbol);
        assert!(mutation.path.is_empty());
        assert_eq!(
            mutation.expected,
            ManagedValue::Unit(UnitLiteral {
                unit: "mm".into(),
                value: prior,
            })
        );
        assert_eq!(mutation.value, replacement);
        if let Some(alias) = alias {
            assert!(
                derive_managed_value_mutation(
                    &current,
                    &SemanticSymbol(alias.into()),
                    &SemanticOutputPath::default(),
                    replacement,
                )
                .is_err(),
                "lexical name does not grant the public parameter's identity"
            );
        }
        assert_eq!(
            current, before,
            "preparation cannot mutate compiler authority"
        );
    }
}
