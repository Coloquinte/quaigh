//! Representation of a techmapping problem

/// Dependency to a mapped signal
pub struct Dependency {
    /// Index of the signal we depend on
    pub index: u32,
    /// Additional delay
    pub delay: u32,
}

/// One mapping choice for a signal
pub struct MappingChoice {
    /// Area cost of taking this choice
    pub area: u32,
    /// Signals it depends on
    pub dependencies: Vec<Dependency>,
}

/// All the choices to map a circuit
pub struct ChoiceGraph {
    required: Vec<u32>,
    choices: Vec<Vec<MappingChoice>>,
}

impl ChoiceGraph {
    pub fn check_solution() {

    }

    pub fn solution_area() {
        
    }

    pub fn solution_delay() {

    }
}