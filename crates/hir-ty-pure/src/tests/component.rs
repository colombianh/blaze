//! A cycle of instances is one component. Both members name the same representative, the
//! component's rows are published by one query, and a second fetch of that query is the same
//! row: the other asker waited on it instead of running the fixpoint again.

use hir_ty::{
    ParamEnvAndCrate,
    next_solver::{GenericArgs, ParamEnv},
};
use hir_ty_mir::mir::dispatch::Body;

use super::fixture::{crate_named, db_with};
use super::fns::fn_named;
use crate::{
    Rule,
    walk::{self, Def, Instance, summary},
};

const CRATE: &str = r#"
//- /lib.rs crate:kernel deps:std
fn lone() -> u32 {
    1
}

fn odd(n: u32) -> u32 {
    if n == 0 { 0 } else { even(n - 1) }
}

fn even(n: u32) -> u32 {
    if n == 0 { 1 } else { odd(n - 1) }
}
"#;

#[test]
fn a_cycle_is_one_component_and_the_second_asker_reads_that_row() {
    let db = db_with(CRATE);
    let krate = crate_named(&db, "kernel");
    let env = ParamEnvAndCrate { param_env: ParamEnv::empty(), krate }.store();
    let instance = |name: &str| Instance {
        def: Def::Body(Body::Fn(fn_named(&db, krate, name))),
        args: GenericArgs::new_from_slice(&[]).store(),
    };
    let (lone, odd, even) = (instance("lone"), instance("odd"), instance("even"));
    blaze_query::db::attach(&db, || {
        let lone_summary = summary(&db, env.clone(), lone);
        assert!(
            !lone_summary.kinds.has(Rule::Recursion),
            "a function that calls nothing is not a cycle"
        );
        let odd_key = walk::component_key(&db, env.clone(), odd.clone()).clone();
        let even_key = walk::component_key(&db, env.clone(), even.clone()).clone();
        assert!(odd_key.cyclic && even_key.cyclic, "odd and even call each other");
        assert_eq!(odd_key.rep, even_key.rep, "both members name one representative");
        let first = walk::component_in(&db, env.clone(), odd_key.rep.clone()) as *const _;
        let second = walk::component_in(&db, env.clone(), even_key.rep.clone()) as *const _;
        assert_eq!(
            first, second,
            "the second asker reads the component row, it does not run another"
        );
        let odd_summary = summary(&db, env.clone(), odd.clone());
        let even_summary = summary(&db, env.clone(), even.clone());
        assert!(odd_summary.kinds.has(Rule::Recursion), "odd calls even which calls odd");
        assert!(even_summary.kinds.has(Rule::Recursion), "even calls odd which calls even");
    });
}
