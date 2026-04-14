// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

//! v2023-09 schema model types.

pub mod parse;

mod job_template;
mod environment_template;
mod parameters;
mod step;
mod environment;
mod actions;
mod host_requirements;
mod task_parameters;
mod constrained_strings;
mod expr_parameters;
pub(crate) mod validate_v2023_09;

// job_template
pub use job_template::JobTemplate;
// environment_template
pub use environment_template::EnvironmentTemplate;
// parameters
pub use parameters::{
    FlexFloat, FlexInt, JobFloatParameterDefinition, JobIntParameterDefinition,
    JobParameterDefinition, JobPathParameterDefinition, JobStringParameterDefinition,
    NullableVec, FileFilter, FloatUserInterface, IntUserInterface, PathUserInterface,
    StringUserInterface,
};
// step
pub use step::{SimpleAction, StepDependency, StepScript, StepTemplate};
// environment
pub use environment::{EmbeddedFile, Environment, EnvironmentScript};
// actions
pub use actions::{Action, CancelationMode, EnvironmentActions, StepActions};
// host_requirements
pub use host_requirements::{AmountRequirement, AttributeRequirement, HostRequirements};
// task_parameters
pub use task_parameters::{
    ChunkIntTaskParameterDefinition, ChunksDefinition, FloatRange, FloatRangeItem,
    FloatTaskParameterDefinition, IntOrFormatString, IntRange, IntTaskParameterDefinition,
    PathTaskParameterDefinition, RangeConstraint, StepParameterSpaceDefinition, StringRange,
    StringTaskParameterDefinition, TaskParameterDefinition,
};
// constrained_strings
pub use constrained_strings::{Description, ExtensionName, Identifier};
// expr_parameters
pub use expr_parameters::{
    BoolUserInterface, BoolValue, HiddenOnlyUserInterface, JobBoolParameterDefinition,
    JobListBoolParameterDefinition, JobListFloatParameterDefinition,
    JobListIntParameterDefinition, JobListListIntParameterDefinition,
    JobListPathParameterDefinition, JobListStringParameterDefinition,
    JobRangeExprParameterDefinition, ListFloatItemConstraints, ListFloatUserInterface,
    ListIntItemConstraints, ListIntUserInterface, ListListIntItemConstraints,
    ListPathUserInterface, ListSimpleUserInterface, ListStringItemConstraints,
    RangeExprUserInterface,
};
