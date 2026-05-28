# qwen mail summarizer prompt pack

This directory contains the prompt contract for alpnest's local mail summarizer.

The target model is currently `qwen3:8b` through Ollama.

This is not fine-tuning. It is prompt-based behavior shaping.

The model's role is intentionally narrow:

```text
mail payload -> faithful dashboard digest
The model is not responsible for final judgement, priority, urgency, motivation, or task planning. Those are handled later through manual ChatGPT review and Alp’s own decisions.

The prompt pack is split into small files so that the behavior can be inspected, edited, and versioned.
