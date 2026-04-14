// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

//! Open Job Description model library for Rust.
//!
//! Provides parsing, validation, and job creation for OpenJD templates
//! conforming to the 2023-09 specification.

pub mod error;
pub mod template;
pub mod job;
pub use job::create_job;
pub use job::step_dependency_graph;
pub use job::step_param_space;
pub use template::parse;
pub mod types;
pub mod capabilities;

// Re-export FormatString and SymbolTable from openjd-expr.
pub use openjd_expr::format_string;
pub use openjd_expr::format_string::FormatString;
pub use openjd_expr::symbol_table;
pub use openjd_expr::symbol_table::SymbolTable;

pub use job::create_job::{preprocess_job_parameters, build_symbol_table, merge_job_parameter_definitions, create_job, convert_environment, evaluate_let_bindings, MergedParameterDefinition};
pub use error::OpenJdError;
pub use parse::{decode_job_template, decode_environment_template, decode_template, DecodedTemplate, DocumentType};
pub use step_param_space::StepParameterSpaceIterator;
pub use step_dependency_graph::StepDependencyGraph;
pub use template::TaskParameterDefinition;
pub use types::{
    DataFlow, EndOfLine, Extensions, FileType, JobParameterInputValues, JobParameterType,
    JobParameterValue, JobParameterValues, KnownExtension, ObjectType, SpecificationRevision,
    TaskParameterSet, TaskParameterType, TaskParameterValue, TemplateSpecificationVersion,
    ValidationContext,
};
