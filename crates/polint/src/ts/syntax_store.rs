//! TS/JS syntax fact store registered on the host [`crate::analysis_api::FactDatabase`].

use std::any::Any;

use crate::analysis_api::{
    FactFamily, FactStore, JsxAttributeFact, StringLiteralFact, TsClassFact, TsComponentFact,
};

/// Syntax facts produced by `polint.ts.syntax` (components/classes/literals/jsx).
#[derive(Debug, Clone, Default)]
pub struct TsSyntaxStore {
    pub ts_components: Vec<TsComponentFact>,
    pub ts_classes: Vec<TsClassFact>,
    pub string_literals: Vec<StringLiteralFact>,
    pub jsx_attributes: Vec<JsxAttributeFact>,
}

impl TsSyntaxStore {
    pub fn ts_components(&self) -> &[TsComponentFact] {
        &self.ts_components
    }

    pub fn ts_classes(&self) -> &[TsClassFact] {
        &self.ts_classes
    }

    pub fn string_literals(&self) -> &[StringLiteralFact] {
        &self.string_literals
    }

    pub fn jsx_attributes(&self) -> &[JsxAttributeFact] {
        &self.jsx_attributes
    }

    pub fn push_ts_component(&mut self, fact: TsComponentFact) -> u64 {
        let run_id = self.ts_components.len() as u64;
        self.ts_components.push(fact);
        run_id
    }

    pub fn push_ts_class(&mut self, fact: TsClassFact) -> u64 {
        let run_id = self.ts_classes.len() as u64;
        self.ts_classes.push(fact);
        run_id
    }

    pub fn push_string_literal(&mut self, fact: StringLiteralFact) -> u64 {
        let run_id = self.string_literals.len() as u64;
        self.string_literals.push(fact);
        run_id
    }

    pub fn push_jsx_attribute(&mut self, fact: JsxAttributeFact) -> u64 {
        let run_id = self.jsx_attributes.len() as u64;
        self.jsx_attributes.push(fact);
        run_id
    }
}

impl FactStore for TsSyntaxStore {
    fn family(&self) -> FactFamily {
        FactFamily::TsComponent
    }

    fn clear(&mut self) {
        self.ts_components.clear();
        self.ts_classes.clear();
        self.string_literals.clear();
        self.jsx_attributes.clear();
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn clone_box(&self) -> Box<dyn FactStore> {
        Box::new(self.clone())
    }
}

/// Registry key used for [`TsSyntaxStore`] in the host fact-store map.
pub const TS_SYNTAX_STORE_FAMILY: FactFamily = FactFamily::TsComponent;
