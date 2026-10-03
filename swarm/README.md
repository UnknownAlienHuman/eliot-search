# Static development-planning metadata

The `swarm/` directory is a historical and advisory map of package boundaries, dependency waves,
architecture coverage and earlier multi-agent planning experiments.

It is **not** an ELIOT Search runtime component and is **not** a repository permission system.

## Current boundary

- Search implementation does not require an issued assignment ticket, materialized context, writer lease,
  acknowledgement, approval/signature profile, submission, review receipt or launch-state transition.
- `launch-state.toml` is a legacy snapshot of an earlier campaign. Its package status arrays cannot
  authorize or block work.
- `orchestration.toml`, ticket/context drafts and template directories describe historical coordination
  ideas only. Do not implement or extend them as a generic controller in this repository.
- Static package, function, module and coverage maps may still be consulted to avoid overlapping writes
  or locate an owner. Architecture and current source remain authoritative when those maps drift.

## External owner

Generic agent orchestration belongs to:

- `UnknownAlienHuman/eliot-swarm-controller` — current standalone prototype for native harness control;
- `UnknownAlienHuman/eliot-memory-os` — future canonical task, WorkScope, Governor, verification and
  finish authority.

Reusable ticket/lease/task/attempt/mailbox/scheduler/check/acceptance functionality must be implemented
there, not in ELIOT Search.

## Permitted use here

Small, read-only metadata may help with:

- package and dependency navigation;
- non-overlapping branch/worktree assignment;
- architecture-coverage audits;
- historical reconstruction of an earlier implementation campaign.

It must not:

- enter product binaries or startup configuration;
- block product coding because a ticket/lease is absent;
- enable or disable Search capabilities;
- own credentials, actor roles or signatures;
- become a database/service/CLI for agent management;
- substitute planning state for compilation or product qualification.

## Product workflow

Read root `AGENTS.md`, ADR 0005, the architecture master and the nearest package documentation. A
maintainer request, issue or PR plus one non-overlapping branch/worktree is sufficient to work on the
product. Product evidence remains governed by the applicable qualification packet, not this directory.
