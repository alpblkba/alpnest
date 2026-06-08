data/projects/mmai/overview.md içine yazılacak içerik

MMAI

status: active
role: course / narrative-bridging project
commit policy: local working state; do not commit by default

purpose

MMAI is treated as a narrative-bridging course project.

The goal is not to fully master every slide on first pass. The goal is to build a connected mental model of the first eight weeks so that later lectures, exercises, and exam preparation become followable.

Main course story:

raw modality input -> representation -> encoder/model -> task head or decoder -> output -> multimodal fusion/generation/agent use-case

current source set

* VL01_SS26_MMAI_Introduction.pdf
* VL02_SS26_MMAI_Text Representation.pdf
* VL03_SS26_MMAI_Natural Language Understaning.pdf
* VL04_SS26_MMAI_Audio_Perception.pdf
* VL05_SS26_MMAI_Visual_Perception_I_v2.pdf
* VL06_SS26_MMAI_Visual_Perception_II.pptx
* VL07_SS26_MMAI_Visual_Perception_III.pdf
* VL8_SS26_MMAI_LLM.pdf
* VL09_SS26_MMAI_Agents.pdf

course narrative

MMAI starts from the observation that real-world AI systems are not purely text, audio, or vision systems. They must perceive, represent, align, generate, and act across modalities.

The first block introduces the system view: unimodal perception and generation are building blocks for multimodal systems.

The language block explains how raw text becomes model input through tokenization, vocabulary construction, feature mapping, sparse vectors, dense vectors, bag-of-words, TF-IDF, and embeddings.

The NLU block uses those text representations for semantic tasks such as sequence classification, sequence labeling, span representation, and contextual modeling with CNNs, RNNs, and self-attention.

The audio block shows how speech becomes text-like symbolic output through signal processing, ASR, CTC, dynamic programming, training, inference, and beam search.

The visual perception block moves from image classification to object detection and segmentation. CNNs learn hierarchical visual features; detection adds localization; segmentation moves to pixel-level understanding.

The LLM block connects understanding to generation through sequence-to-sequence modeling, decoder-only models, encoder-decoder models, cross-attention, RNNs, self-attention, and autoregressive generation.

The agents block closes the loop by turning language models into systems that can remember dialog context, use actions, reason, plan, query tools, and interact with an environment.

working principle

For each lecture, track five questions:

1. What is the raw input?
2. What representation is built?
3. What model family processes it?
4. What output/task is produced?
5. How does this connect to multimodal AI?

current priority

Today is a bridging day.

Target outcome:

By the end of the day, I should be able to explain the whole first-eight-week narrative from introduction to agents without needing to reproduce every detail.

do not commit

This file is local course/project state. Do not commit unless explicitly requested.
