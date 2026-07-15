# ADR 0015: Distinguish declarative views, artifacts, and full Lattice Apps

## Status
Accepted

## Context
Users and agents need both safe no-code dashboards and unrestricted web application development without making every simple interface a React project.

## Decision
Provide a hierarchy:
1. built-in blocks and views;
2. declarative dashboards/interfaces;
3. HTML/CSS/JavaScript artifacts;
4. full Lattice App source projects using any web framework;
5. external web embeds.

React is the blessed app template, not a required artifact format. Ship a UI kit and host SDK. Every app runs behind explicit capabilities and isolation.

## Consequences
- Simple resources stay simple.
- Full web ecosystem flexibility is available when needed.
- Build provenance, publishing, sandboxing, and lifecycle management are required.
