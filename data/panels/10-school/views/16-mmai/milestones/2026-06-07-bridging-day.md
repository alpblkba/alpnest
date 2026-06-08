2026-06-07 MMAI bridging day

status: planned
type: 8-hour narrative bridge
commit policy: local working state; do not commit by default

goal

Build a connected understanding of the first uploaded MMAI source set.

This is not a full memorization day.

Success means I can explain the flow:

introduction -> text representation -> NLU -> audio perception -> visual classification -> detection -> segmentation -> LLM/NLG -> agents

time blocks

block 1 — 0:00–0:30 — course map

Source:

* VL01_SS26_MMAI_Introduction.pdf

Task:

Read the schedule, organization, and lecture block overview.

Output:

Write the course map in one paragraph.

Checkpoint:

I can explain why MMAI exists and how unimodal systems become multimodal systems.

block 2 — 0:30–1:30 — text representation

Source:

* VL02_SS26_MMAI_Text Representation.pdf

Task:

Build the chain from raw text to vector representation.

Output:

Write short definitions for tokenization, vocabulary, BPE, one-hot, dense vectors, Word2Vec, bag-of-words, and TF-IDF.

Checkpoint:

I can explain why text needs representation before NLU or LLMs.

block 3 — 1:30–2:30 — NLU

Source:

* VL03_SS26_MMAI_Natural Language Understaning.pdf

Task:

Connect text representations to semantic tasks.

Output:

Write one example each for sequence classification, sequence labeling, and span representation.

Checkpoint:

I can explain the role of CNN, RNN, and self-attention as sequence layers.

block 4 — 2:30–3:30 — audio perception

Source:

* VL04_SS26_MMAI_Audio_Perception.pdf

Task:

Understand ASR as speech-to-symbol/text mapping.

Output:

Write the ASR chain and explain CTC in simple words.

Checkpoint:

I can explain why audio alignment is hard and why CTC/dynamic programming are useful.

block 5 — 3:30–4:45 — visual perception I

Source:

* VL05_SS26_MMAI_Visual_Perception_I_v2.pdf

Task:

Understand CNN-based image classification.

Output:

Write the CNN image-classification chain and summarize VGG, GoogLeNet, and ResNet in one sentence each.

Checkpoint:

I can explain why residual connections allow deeper CNNs.

block 6 — 4:45–5:45 — visual perception II

Source:

* VL06_SS26_MMAI_Visual_Perception_II.pptx

Task:

Move from classification to detection.

Output:

Write the difference between classification, localization, and detection.

Checkpoint:

I can explain objectness, bounding boxes, and why detection is “what and where”.

block 7 — 5:45–6:45 — visual perception III

Source:

* VL07_SS26_MMAI_Visual_Perception_III.pdf

Task:

Move from detection to segmentation.

Output:

Write the difference between image, semantic, instance, panoptic, and interactive segmentation.

Checkpoint:

I can explain why segmentation is pixel-level understanding.

block 8 — 6:45–7:30 — LLM / NLG

Source:

* VL8_SS26_MMAI_LLM.pdf

Task:

Connect NLU to generation.

Output:

Write the difference between decoder-only and encoder-decoder generation.

Checkpoint:

I can explain autoregressive generation, self-attention, and cross-attention.

block 9 — 7:30–8:00 — agents

Source:

* VL09_SS26_MMAI_Agents.pdf

Task:

Close the system loop.

Output:

Write the difference between LLM and LLM agent.

Checkpoint:

I can explain perception, dialog state, reasoning/planning, action, and environment feedback.

final deliverable

At the end of the session, produce one page titled:

“MMAI first source set: narrative bridge”

It should answer:

1. What is the common pipeline across text, audio, and vision?
2. How does representation learning connect the course?
3. How do perception and generation differ?
4. Why do detection and segmentation matter for multimodal AI?
5. What turns an LLM into an agent?

minimum viable success

Even if time slips, finish these:

* introduction map
* text representation chain
* NLU task taxonomy
* ASR/CTC intuition
* CNN/detection/segmentation progression
* LLM generation intuition
* LLM agent difference

do not commit

This milestone is local planning state.
