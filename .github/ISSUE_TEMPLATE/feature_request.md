---
name: Feature Request
about: Suggest a new computation idea, or any other feature
title: "feat: "
labels: enhancement
assignees: ""
---

### Motivation

Describe your motivation on requesting this feature. How does it extend the Dria Knowledge Network, or the compute node itself?

### Technical Requirements

What is required for the node to expect the requirements of this request? Some examples:

- Does it require a GPU?
- Does it require an API key for some third party service?
- How much RAM will this feauture require?
- Does this feature require an additional container within the compose file?

### Task Input

Describe clearly what the input for this task is. For example, Synthesis tasks take in a prompt as an input, and this is instantiated as:

```rs
type SynthesisPayload = TaskRequestPayload<String>;
```
