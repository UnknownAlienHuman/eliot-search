"""Retained Python compatibility primitives for the still-unported materialization planner.

The executable context-artifact builder is Rust-owned. No new builder logic may
be added here; `core.py` and `bundle.py` remain only until the context-
materialization planner and its legacy qualification corpus are moved to Rust.
"""
