# Tile Conservative Open License - Commercial (TCOL-COM)
Version 1.0

This Tile Conservative Open License - Commercial ("TCOL-COM" or "License") is a highly restrictive, patent-fortified derivative of the standard TCOL. It is exclusively designed to govern and protect physical hardware intellectual property (IP), specifically Instruction Set Architectures (ISA) such as the **H16B (Heterogeneous 16 Bytes)** and its extensions like **AGCX (AltiGeo)**, developed by Middle Eastern Calculation Machinary Ltd and the Tile Open Source Software Foundation.

By exercising any rights granted under this License, you (the "Licensee") accept and agree to be bound by its terms and conditions.

## 1. DEFINITIONS
1.1. **"Middle Eastern Calculation Machinary Ltd"** refers to the original creators, licensors, and right-holders of this hardware ecosystem and architecture.
1.2. **"Chief Architect"** refers to Efe İlhan Yüce, the original architect and visionary of the architecture.
1.3. **"Covered Hardware Architecture"** refers to the original specifications, blueprints, SystemVerilog designs, microcode, or any physical or emulated implementations distributed under this License, including but not limited to the H16B ISA, dual-pipeline structures, and AGCX (AltiGeo).
1.4. **"Proprietary Technologies"** specifically refers to Middle Eastern Calculation Machinary Ltd's patented or proprietary methodologies, including but not limited to the "Sözel Matematik" execution pipelines, Altitudal Geometry Calculation Extensions (AGCX), and the 128-bit MultiVec (MVX) paradigms.
1.5. **"Commercial Use"** means any use intended for commercial advantage or monetary compensation. This includes, but is not limited to, manufacturing physical silicon (CPUs, GPUs, SoCs, ASICs, FPGAs), incorporating the architecture into a product for sale/lease, providing commercial cloud/emulation services (SaaS, IaaS), or using the architecture for commercially funded derivative designs.

## 2. ACADEMIC AND INTERNAL FOUNDATION USE
Subject to the terms and conditions of this License, each Contributor hereby grants You a worldwide, royalty-free, non-exclusive, perpetual license to use, study, simulate, and emulate the Covered Hardware Architecture STRICTLY FOR:
a) Academic research, non-profit educational purposes, and peer review.
b) Internal development within the Tile Open Source Software Foundation ecosystem.

## 3. STRICT PROHIBITION ON COMMERCIAL USE
3.1. **No Commercial License Granted:** ANY AND ALL COMMERCIAL USE OF THE COVERED HARDWARE ARCHITECTURE IS STRICTLY PROHIBITED WITHOUT AN EXPLICIT, WRITTEN COMMERCIAL LICENSE AGREEMENT.
3.2. **Commercial Agreements:** To utilize the Covered Hardware Architecture (including H16B and AGCX) for any Commercial Use, the entity MUST negotiate and execute a separate **TCOL Commercial Agreement** with Middle Eastern Calculation Machinary Ltd / Tile Foundation. This agreement will dictate royalty structures, auditing rights, and explicit patent grants.

## 4. PATENT PROTECTIONS AND TÜRKPATENT
The Covered Hardware Architecture intricately incorporates Middle Eastern Calculation Machinary Ltd' Proprietary Technologies.
4.1. **Patent Rights:** Middle Eastern Calculation Machinary Ltd retains all rights, titles, and interests in all patents, trademarks, and intellectual property associated with the Covered Hardware Architecture.
4.2. **TÜRKPATENT Enforcement:** Patent rights and protections granted and enforced under this License (especially regarding the H16B ISA and AGCX) are strictly governed by the Turkish Industrial Property Code (Sınai Mülkiyet Kanunu) No. 6769 and applicable international treaties recognized by the Turkish Patent and Trademark Office (**TÜRKPATENT**).
4.3. **Patent Retaliation:** Any unauthorized commercial implementation, tape-out, or fabrication of the Covered Hardware Architecture constitutes direct and willful patent infringement under TÜRKPATENT regulations and international patent law.

## 5. ZERO-TRUST ARCHITECTURE, INTEGRITY, AND PROHIBITION OF BACKDOORS
The H16B ISA is designed to be deterministic and completely transparent to the bare-metal OS (HXNU/heterexec).
5.1. **Prohibition of Hardware Backdoors:** Any entity fabricating or emulating the Covered Hardware Architecture is strictly prohibited from intentionally introducing hardware backdoors (e.g., hidden microcode instructions, shadow registers, undocumented management engines) designed to bypass the SECinter security layer.
5.2. **Criminal Liability under the Turkish Penal Code (TCK):** The deliberate insertion of such malicious structures into the physical or emulated silicon shall be deemed a direct violation of the Turkish Penal Code (Türk Ceza Kanunu - TCK), specifically Articles 243, 244, and 245/A, and will be prosecuted accordingly.

## 6. DATA PRIVACY AND TELEMETRY (KVKK COMPLIANCE)
6.1. **KVKK Mandate:** Any collection, processing, transmission, or storage of telemetry, hardware usage metrics, crash dumps, or user data derived from hardware executing the Covered Hardware Architecture must strictly comply with the Turkish Personal Data Protection Law (Kişisel Verilerin Korunması Kanunu - **KVKK**) No. 6698, as well as applicable international equivalents (e.g., GDPR).
6.2. **Hardware-Level Opt-In:** The Covered Hardware Architecture shall not contain mandatory, non-disablable telemetry reporting mechanisms (such as unpatchable hardware monitoring rings sending data externally). All telemetry must be strictly opt-in and explicitly controllable by the end-user or the bare-metal OS.

## 7. MANDATORY ATTRIBUTION
Any physical, emulated, or documented distribution of the Covered Hardware Architecture for academic/non-commercial use MUST include clear and prominent attribution.
7.1. **Documentation Attribution:** Any technical manual, datasheet, or academic paper discussing implementations of the architecture MUST explicitly state: 
"This hardware incorporates the H16B Instruction Set Architecture, originally designed by the Chief Architect at Middle Eastern Calculation Machinary Ltd."

## 8. NO WARRANTY & LIMITATION OF LIABILITY
8.1. **"AS IS" Basis:** The Covered Hardware Architecture specifications are provided under this License on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND.
8.2. **Exclusion of Damages:** Under no legal theory shall Middle Eastern Calculation Machinary Ltd, the Chief Architect, or any Contributor be liable to You for damages (including physical hardware damage, fabrication defects, or commercial losses) arising out of the use or inability to use the Covered Hardware Architecture.

## 9. GOVERNING LAW AND JURISDICTION
9.1. **Turkish Law:** This License, and any disputes arising from or related to it, shall be governed by and construed in accordance with the laws of the Republic of Turkey.
9.2. **Exclusive Jurisdiction:** Any legal action or proceeding arising under this License will be brought exclusively in the federal, regional, or local courts located within the Republic of Turkey.

## 10. ENTIRE AGREEMENT
This License represents the complete agreement concerning the subject matter hereof. No modifications to this License are valid unless explicitly sanctioned in writing by Middle Eastern Calculation Machinary Ltd.

---
**END OF TERMS AND CONDITIONS**
