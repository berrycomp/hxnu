# nxutils — HXNU Userspace Utilities

## Lisanslama

**nxutils**: TCOL (Tile Conservative Open License) — HPL ile uyumlu
- Temel alınan rustybox MIT kodu: MIT lisansı korunur (attribution)
- Yeni yazılan tüm kod, entegrasyon, syscall katmanı: **TCOL**
- MIT → TCOL geçişi yasal: MIT sublicensing'e izin verir

HXNU ekosisteminde TCOL ile sabitlenecek programlar:
- nxutils (sh, cat, ls, echo, play, ...)
- user-init / init-zero
- heterexec
- HFS araçları
- G-Boot

## Özet

`nxutils`, rustybox MIT edition'ın forku olacak ve
POSIX/Linux syscall katmanına **bağımlı olmadan** doğrudan
HXNU native syscall ABI'yi (`HxSYS_*`) kullanacak.

## Mimari

```
┌─────────────────────────────────────────┐
│           Userspace Programs            │
├──────────────────┬──────────────────────┤
│    nxutils       │   musl/Linux binary  │
│  (HxSYS_* ABI)  │   (LCL üzerinden)    │
│  [TCOL]          │   [kendi lisansı]    │
├──────────────────┴──────────────────────┤
│              HXNU Kernel [TCOL]         │
│   ┌──────────────┐  ┌────────────────┐  │
│   │  HxSYS_*     │  │  LCL module    │  │
│   │  Native ABI  │  │  (LINUX_SYS_*) │  │
│   └──────────────┘  └────────────────┘  │
│           heterexec bridge [TCOL]       │
└─────────────────────────────────────────┘
```

## Planlanan HxSYS_* tablosu

| No  | İsim              | Açıklama                        |
|-----|-------------------|---------------------------------|
|  0  | HxSYS_EXIT        | Süreci sonlandır                |
|  1  | HxSYS_WRITE       | Handle'a yaz                    |
|  2  | HxSYS_READ        | Handle'dan oku                  |
|  3  | HxSYS_OPEN        | Dosya/cihaz aç                  |
|  4  | HxSYS_CLOSE       | Handle kapat                    |
|  5  | HxSYS_YIELD       | Zamanlayıcıya bırak             |
|  6  | HxSYS_GETPID      | Süreç ID'si                     |
|  7  | HxSYS_AUDIO_PLAY  | PCM verisi çal (ALC272)         |
|  8  | HxSYS_AUDIO_STOP  | Sesi durdur                     |
|  9  | HxSYS_FB_BLIT     | Framebuffer'a blit              |
| 10  | HxSYS_HXEXT_CALL  | Driver fonksiyonu çağır         |
| 11  | HxSYS_IPC_SEND    | heterexec IPC mesajı gönder     |
| 12  | HxSYS_IPC_RECV    | heterexec IPC mesajı al         |

## LCL (Linux Compat Layer)

- Ayrı bir `.hxext` modülü olarak yüklenir
- `LINUX_SYS_*` numaralarını `HxSYS_*`'e çevirir
- musl ile derlenmiş programlar için gerekli
- mevcut rustybox (Linux-musl) LCL olmadan çalışmaz

## nxutils Roadmap

1. **Fork**: rustybox MIT → `nxutils` repo (Middle Eastern Calculation Machinary Ltd)
2. **Lisans**: TCOL (MIT attribution header korunur)
3. **Syscall katmanı**: `src/syscall.rs` — doğrudan HxSYS_*
4. **İlk araçlar**: sh, cat, ls, echo, pwd, play
5. **Build hedefi**: `x86_64-unknown-none` (no_std + HXNU ABI)
6. **Ses testi**: `play /audio/driver-test.wav` → HxSYS_AUDIO_PLAY

## Notlar

- POSIX standardı: LCL modülüyle gelir, kernel'e gömülü değil
- nxutils: TCOL lisanslı, ekosisteme tam entegre
- Yakın vade: HxSYS_AUDIO_PLAY kernel'e eklenmeli
