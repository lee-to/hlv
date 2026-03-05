/// Test spec parsed from validation/test-specs/*.md
#[derive(Debug, Clone)]
pub struct TestSpec {
    pub contract_id: String,
    pub contract_version: Option<String>,
    pub derived_from: Option<String>,
    pub sections: Vec<TestSection>,
}

#[derive(Debug, Clone)]
pub struct TestSection {
    pub category: TestCategory,
    pub tests: Vec<TestCase>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TestCategory {
    Contract,
    PropertyBased,
    EdgeCase,
    Performance,
    Security,
}

impl std::fmt::Display for TestCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Contract => write!(f, "Contract Tests"),
            Self::PropertyBased => write!(f, "Property-Based Tests"),
            Self::EdgeCase => write!(f, "Edge Case Tests"),
            Self::Performance => write!(f, "Performance Tests"),
            Self::Security => write!(f, "Security Tests"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TestCase {
    pub id: String,
    pub title: String,
    pub gate: Option<String>,
    pub body: String,
}
