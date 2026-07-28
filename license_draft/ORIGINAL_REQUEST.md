# Original User Request

## Initial Request — 2026-07-01T02:45:05+03:00

# Teamwork Project Prompt — Draft

> Status: Launched
> Goal: Craft prompt → get user approval → delegate to teamwork_preview

[Project description: MPL 2.0 temel alınarak, HXNU işletim sistemi ve patentli teknolojilerini (örn. SXRC, Ray-Tree) ticari sömürüden korumak amacıyla, zayıf copyleft (weak copyleft) özelliklerini koruyan ancak patent haklarını daha katı bir şekilde savunan 'HXNU Public License' (HPL) isimli özel bir açık kaynak lisans metni taslağı oluşturmak.]

Working directory: /home/eilhanzy/Projects/hxnu/license_draft
Integrity mode: development

## Requirements

### R1. Temel Lisans Çerçevesi ve Defansif Patent Koruması
Lisans metni, MPL 2.0 yapısını (dosya bazlı zayıf copyleft) baz almalı ve ticari kullanımı serbest bırakmalıdır. Ancak güçlü bir "Defansif Patent Feshi" (Patent Retaliation) maddesi içermeli; projeyi kopyalayıp Berrycomp'a patent davası açanların veya patentleri izinsiz kullananların lisansı anında iptal edilmelidir.

### R2. Marka Hakları (Trademark) Koruması
HXNU, Neonix ve Berrycomp isimlerinin, lisanslı koddan türetilen başka ticari projelerde veya ürünlerin pazarlama/reklam faaliyetlerinde kullanılması kesin bir dille yasaklanmalıdır.

### R3. Akademik Atıf Koşulları
Lisans metnine özel bir akademik madde eklenmelidir: Kodun veya patentli algoritmaların (SXRC, Ray-Tree vb.) akademik araştırmalarda kullanılması durumunda, ilgili teorilerin ve projenin yayınlarda açıkça referans/atıf olarak gösterilmesi zorunlu kılınmalıdır.

## Acceptance Criteria

### Yasal ve Yapısal Uygunluk
- [ ] Lisans metni, profesyonel yasal İngilizce (Legal English) kullanılarak yazılmalı ve "HXNU Public License (HPL)" başlığını taşımalıdır.
- [ ] Zayıf copyleft kuralı, marka yasağı, patent feshi ve akademik atıf gereksinimlerinin her biri ayrı, açıkça numaralandırılmış maddeler halinde metinde yer almalıdır.
- [ ] İçerik, yasal bir boşluk yaratmayacak düzeyde net olmalı, "agent-as-judge" (bağımsız denetleyici ajan) tarafından MPL 2.0 ile uyumluluğu doğrulanabilir olmalıdır.
- [ ] Nihai çıktı, `HPL-1.0.md` veya `.txt` formatında, belirtilen çalışma dizinine kaydedilmelidir.
