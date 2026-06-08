MMAI notes

core mental model

MMAI is about turning multiple raw data types into useful representations, then connecting those representations for perception, generation, fusion, and action.

The shared pipeline:

raw input -> representation -> encoder/model -> task output -> multimodal use

lecture 01: introduction

Main idea:

AI systems are increasingly multimodal because the world itself is multimodal.

Important distinction:

* unimodal: one data type, for example text only
* multimodal: several data types, for example text, audio, image, video, action

Course blocks:

* language perception
* visual perception
* language generation
* video/action and image generation
* multimodal perception and generation
* advanced topics

Personal takeaway:

Introduction is the map. It tells me why text, audio, and vision are not isolated topics but building blocks of one multimodal system.

lecture 02: text representation

Main idea:

Text must be converted into numerical representations before models can process it.

Core chain:

text -> tokenization -> vocabulary -> feature mapping -> vector representation

Important concepts:

* tokenization
* segmentation
* vocabulary
* byte pair encoding
* one-hot encoding
* dense vectors
* Word2Vec
* bag-of-words
* TF-IDF

Personal takeaway:

Text representation is the root of the language side. NLU, LLMs, dialog systems, and multimodal text-image alignment all depend on this layer.

Key questions:

* Why do we tokenize?
* Why is vocabulary construction non-trivial?
* Why is one-hot limited?
* What do dense vectors fix?
* Why does TF-IDF still matter?

lecture 03: natural language understanding

Main idea:

NLU maps text representations to semantic outputs.

Core tasks:

* sequence classification
* sequence labeling
* span representation
* neural sequence labeling

Model families:

* CNN
* RNN
* self-attention

Personal takeaway:

NLU is where text representation becomes meaning for a task. The important jump is from static token vectors to contextual representations.

Key questions:

* When is a task sequence classification?
* When is a task sequence labeling?
* Why do we need sequence layers?
* What does self-attention solve compared with RNNs?

lecture 04: audio perception

Main idea:

Audio perception maps speech signal to symbolic or linguistic output.

Core chain:

speech waveform -> signal/acoustic features -> sequence model -> aligned output -> decoded text

Important concepts:

* speech processing
* ASR
* CTC
* dynamic programming
* training
* inference
* beam search

Personal takeaway:

Audio is a modality bridge into language. ASR connects speech to text pipelines, but alignment makes it hard.

Key questions:

* Why is speech alignment difficult?
* What does CTC solve?
* Why do we need dynamic programming?
* Why is beam search useful?

lecture 05: visual perception I

Main idea:

CNNs learn visual representations for image classification.

Core chain:

image -> convolutional layers -> hierarchical features -> classifier -> class label

Important concepts:

* convolution kernels
* spatial structure
* ImageNet
* VGG
* GoogLeNet
* ResNet
* residual connections

Personal takeaway:

Visual encoders are the backbone of later multimodal vision-language systems. The main historical jump is from handcrafted features to learned hierarchical features.

Key questions:

* Why are CNNs suitable for images?
* What does depth buy us?
* Why did VGG, GoogLeNet, and ResNet matter?
* Why do residual connections help optimization?

lecture 06: visual perception II

Main idea:

Detection extends classification with localization.

Core task progression:

classification -> localization -> detection

Important concepts:

* bounding boxes
* objectness
* sliding window
* detection as regression
* detection as classification
* region proposals
* R-CNN-style detection
* multimodal fusion connection

Personal takeaway:

Detection answers “what and where?” This is crucial for grounding text in images.

Key questions:

* Why is detection harder than classification?
* What does objectness mean?
* Why are bounding boxes regression outputs?
* Why do region proposals help?

lecture 07: visual perception III

Main idea:

Segmentation moves from box-level understanding to pixel-level understanding.

Segmentation types:

* image segmentation
* semantic segmentation
* instance segmentation
* panoptic segmentation
* interactive segmentation

Important concept:

* SAM and prompt-based segmentation

Personal takeaway:

Segmentation answers “what and where exactly?” It is more precise than detection and important for embodied, medical, interactive, and video-based AI.

Key questions:

* What is the difference between semantic and instance segmentation?
* Why does panoptic segmentation combine both?
* What makes interactive segmentation different?
* Why is SAM important?

lecture 08: LLM / natural language generation

Main idea:

Language generation maps context to variable-length output.

Core chain:

input/context -> context representation -> decoder/generator -> next-token prediction -> generated sequence

Important concepts:

* text generation
* conditional text generation
* sequence-to-sequence
* decoder-only models
* encoder-decoder models
* cross-attention
* RNN
* self-attention
* autoregressive generation

Personal takeaway:

LLMs are not just NLU models. They generate output token by token and can become the language interface of multimodal systems.

Key questions:

* What is sequence-to-sequence modeling?
* What is decoder-only generation?
* What does cross-attention connect?
* Why is self-attention central?

lecture 09: agents

Main idea:

An agent perceives the environment and performs actions.

Core chain:

perception -> dialog/context state -> reasoning/planning -> action -> environment feedback

Important concepts:

* agent
* sensors
* actuators
* goal-oriented dialog
* social dialog
* rule-based dialog systems
* LLM agents
* multi-turn context
* actions
* reasoning and planning
* dialog state

Personal takeaway:

LLM agents close the multimodal loop. They combine perception, memory, language, action, and environment interaction.

Key questions:

* What is the difference between LLM and LLM agent?
* Why does multi-turn context matter?
* Why does action space matter?
* What is dialog state?

one-sentence whole-course bridge

MMAI teaches how text, audio, and vision are converted into representations, processed by neural models, connected through generation/fusion mechanisms, and finally used by agents that can interact with the world.
