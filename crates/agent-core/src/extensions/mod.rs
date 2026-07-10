
// WindWave Extensions Module
// Inspired by Ruflo's architecture
// 
// This module contains components inspired by Ruflo's design:
// - Plugin System (ADR-0001)
// - Namespace System (ADR-0002)
// - Layered Controller Registry (ADR-0003)

pub mod namespace;
pub mod controller;
pub mod plugin;

pub use namespace::{Namespace, NamespaceError, builder};
pub use controller::{
    Controller, ControllerRegistry,
    ControllerContext, ControllerError,
    InitLevel,
};
pub use plugin::{Plugin, PluginRegistry, PluginContext, PluginError};
