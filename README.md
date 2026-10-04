# 사립 저스티스 학원 열혈청춘일기 2 한국어 패치

PlayStation 일본어판 **사립 저스티스 학원 열혈청춘일기 2**의 비공식 한국어 패치입니다.

현재 버전은 **v1.0.0**입니다. 기존 v0.1.0과 패치 내용은 같습니다.

**[패치 다운로드](https://github.com/mcpads/justice-gakuen2-kr-patch/releases/tag/v1.0.0)** · **[문제 제보](https://github.com/mcpads/justice-gakuen2-kr-patch/issues)**

## 공개 내용

이 저장소는 배포용 xdelta 패치(릴리스)와 패처 소스 코드를 제공합니다. 원본 게임, 패치가 적용된 디스크 이미지, 번역·그래픽 자산과 글꼴 파일은 제공하지 않습니다.

대사·메뉴·그래픽 문구를 한국어로 옮기고, 한글 이름 입력과 표시, 화면별 글꼴 및 숫자 배치를 적용했습니다. 원래 영어로 표시되던 문구는 영어를 유지합니다.

## 적용 대상

본인이 보유한 원본에서 준비한 아래 BIN 파일에 적용합니다. 파일 이름이 같아도 해시가 다르면 적용 대상이 아닙니다.

| 항목 | 값 |
| --- | --- |
| 게임 | Shiritsu Justice Gakuen - Nekketsu Seishun Nikki 2 (Japan) |
| 게임 ID | SLPS-02120 |
| 형식 | 단일 트랙 BIN/CUE, MODE2/2352 |
| 원본 BIN 크기 | 663,216,960바이트 |
| 원본 BIN SHA-256 | `62dbc6ca47ec8d9dfbb5797f35d4e720d63e00b5a8e0edd080b149c3ff4f1823` |

다른 지역판, ISO/CHD 파일, 이미 패치된 BIN에는 바로 적용하지 마세요. 먼저 위 형식과 해시에 맞는 원본 BIN을 준비해야 합니다.

## 패치 적용

1. 원본 BIN/CUE와 메모리 카드 파일을 별도로 보관합니다.
2. 릴리스에서 `justice-gakuen2-kr-v1.0.0.xdelta`을 받습니다.
3. [xdelta3](https://github.com/jmacd/xdelta/releases)를 준비하고 원본 BIN의 SHA-256을 위 표와 비교합니다. 배포 패치는 xdelta3 3.2.0으로 생성·재적용 검증했습니다.
4. 아래 명령으로 새 BIN을 만듭니다. `original.bin`은 본인의 원본 BIN 파일명으로 바꾸세요. 경로에 공백이 있으면 큰따옴표로 감쌉니다.

```sh
xdelta3 -d -s "original.bin" "justice-gakuen2-kr-v1.0.0.xdelta" "justice-gakuen2-kr.bin"
```

5. 결과 BIN의 SHA-256이 아래 값과 일치하는지 확인합니다.
6. 같은 폴더에 `justice-gakuen2-kr.cue`를 텍스트 파일로 만들고 아래 내용을 저장합니다. 에뮬레이터에서 이 CUE를 엽니다.

```cue
FILE "justice-gakuen2-kr.bin" BINARY
  TRACK 01 MODE2/2352
    INDEX 01 00:00:00
```

### 파일 확인

| 파일 | 크기 | SHA-256 |
| --- | --- | --- |
| xdelta 패치 | 65,968,303바이트 | `0ddd4d98f4089b14f22fe628730b1c7f45b25cc898701da8bc2d1d44cc31d69c` |
| 적용된 BIN | 663,216,960바이트 | `a9f2bd73a9b6e0ff62feb043d8445a693a4fa9a80ffc29c273d4670bba09fddb` |

SHA-256은 다음 명령으로 확인할 수 있습니다. 파일명을 바꾸어 원본·패치·결과를 각각 확인하세요.

```powershell
# Windows PowerShell
Get-FileHash -Algorithm SHA256 "original.bin"
```

```sh
# macOS
shasum -a 256 "original.bin"
# Linux
sha256sum "original.bin"
```

## 이전 0.1.0 주요 변경 사항

- 영문·숫자·기호가 포함된 이름과 별명이 실기시험 준비 및 대전 화면에서 잘못 표시되던 문제를 수정했습니다.
- 시험 결과 화면의 제목과 안내 문구 뒤에 사각형 배경이 드러나던 문제를 수정했습니다.
- 트레이닝에서 캐릭터를 다시 선택한 뒤 공격할 때 멈추던 문제를 수정했습니다.
- 메인 메뉴와 설명, 동아리명, 보너스·열혈비디오 안내 등의 글꼴과 배치를 다듬었습니다.
- 대사와 메뉴의 번역·기호·간격을 보완하고, 카드·편지·이벤트 배경 등의 한국어 그래픽을 개선했습니다.

버전을 바꿀 때는 메모리 카드를 백업하고, 이전 버전의 세이브 스테이트 대신 게임을 새로 부팅해 게임 내 불러오기를 사용하세요. 이전 버전에서 잘못된 이름으로 등록된 EDIT 캐릭터는 다시 등록해야 할 수 있습니다.

## 소스 코드

원본 CUE/BIN 검증, ISO 9660 재배치, Mode 2 EDC/ECC 재생성, 게임 고유 LZ 압축과 TZZ·TIM 처리, 글리프 배정과 폰트 래스터화, R3000A 코드 훅 생성을 담은 Rust 코드입니다. 번역·그래픽 입력(`assets/`, `specs/`)과 글꼴이 있어야 디스크를 만들 수 있습니다.

### 빌드와 테스트

```bash
cargo build --release
cargo test
```

Rust 1.98.0(`rust-toolchain.toml`)을 사용합니다. 기본 테스트는 저장소 안의 코드와 합성 입력만 사용합니다. 원본 디스크, 폰트, 빌드 자산이 필요한 테스트는 `#[ignore = "requires ..."]`에 필요한 입력을 밝혀 두었습니다. 아래 입력을 배치한 뒤 `cargo test -- --ignored`로 실행하며, 입력이 없으면 성공으로 넘어가지 않고 실패합니다. 이름 입력 빌드 결과가 필요한 두 테스트는 `JUSTICE_NAME_INPUT_DIR`에 `build-dialogue-disc` 출력의 `name-entry` 디렉터리를 지정해야 합니다.

### 빌드 입력

명령은 저장소 루트에서 실행합니다.

| 입력 | 위치 | 비고 |
| --- | --- | --- |
| 원본 디스크 | `roms/Shiritsu Justice Gakuen - Nekketsu Seishun Nikki 2 (Japan).{cue,bin}` | 빌드는 `--cue`로 지정하고, 테스트는 이 경로를 읽음 |
| 빌드 자산 | `assets/` | 빌드 spec `assets/build/development.json`, 번역, 한글 그래픽 원화 (비공개) |
| 화면 경로 spec | `specs/` | 빌드 spec이 참조 (비공개) |
| 폰트 | `../fonts/` | 저장소의 상위 디렉터리. 빌드 spec이 파일 경로와 SHA-256을 고정 |

빌드 spec은 `<자산 루트>/build/development.json`에 있어야 합니다. spec을 읽으면 `<자산 루트>`가 정해지고, 별도 spec 항목이 없는 공용 manifest도 그 아래에서 읽습니다.

배포 패치 v1.0.0은 다음 폰트 파일로 만들었으며, 빌드는 SHA-256이 다르면 진행하지 않습니다.

| 폰트 | 배포처 |
| --- | --- |
| 갈무리 (Galmuri7·9·11·11 Bold·11 Condensed·14) | [Galmuri](https://github.com/quiple/galmuri) |
| Neo둥근모 | [neodgm](https://github.com/neodgm/neodgm) |
| 달무리 | [Dalmoori](https://github.com/RanolP/dalmoori-font) |
| 메이플스토리 Bold | [메이플스토리 서체](https://maplestory.nexon.com/Media/Font) |
| 넥슨 Lv.2 고딕·Lv.2 고딕 Bold, 넥슨 풋볼고딕 B | 넥슨 |
| 던파 연단된 칼날 Medium·Bold | 넥슨 |
| DenkiChip Hangul (x10y12px), MaruMinya Hangul (x12y12px) | 각 배포처 |

```text
3882bd35066c26b0392cd4963ff9b3c151041dec34adc9d5633d137d1d9b9855  galmuri/Galmuri7.ttf
5cb68052ee0a15571747e91c20f145e24b51bb459c6cd58226fafee78d9c0b16  galmuri/Galmuri9.ttf
2c709890595668f7bdb6df408420fda957dde0288e95b31a1cc17a2ab98b4b4f  galmuri/Galmuri11.ttf
5265b2f437fe81f0c8095b44c0173dd9a276b58a42552bf983f21c0e69e6e8af  galmuri/Galmuri11-Bold.ttf
7b433b4a007c36dfb535fdea11de3e4f4c8b641ab591ed05d3bc0a4bbd75eb5f  galmuri/Galmuri11-Condensed.ttf
d3818c0f2898a3b2d79ccd04ec1e4de5e8940aa26abee261f73e315a44ce8df9  galmuri/Galmuri14.ttf
d61b60eccb731f8ca9c7da582e4a05a94db66b570471809950aa9a7261b941d6  neodgm/neodgm.ttf
ba40b8f9ada002005d1f5f3676a82b7bff07ddbd186a923073101099131999e6  dalmoori-font/dalmoori.ttf
d57eaff48a793ff872a0f33bba2943d058d07c81ed64c68054858a287b85811a  NEXON_Maplestory/TTF/Maplestory Bold.ttf
389ad546769c0cb958b1c5c5c1d4b473867b433e0a6697b01907c7d7e1565c60  NEXON_Lv2_Gothic/TTF/NEXON Lv2 Gothic.ttf
7d538770c259b287b1e94d3e4fcb55bac71d866dbc98a69e0aab1fbfb7c6dade  NEXON_Lv2_Gothic/TTF/NEXON Lv2 Gothic Bold.ttf
37cb6eb268d6f7d45a1467c0deac6eb2fbb80e930f8e66d528494f296f571ec9  NEXON_Football_Gothic/OTF/NEXON Football Gothic B.otf
364ea508e7f58114052406369bc9c6e5c7dfdc09e3693bd20f14ce35aef66af9  DNF_ForgedBlade/TTF/DNFForgedBlade-Medium.ttf
c906b3ab3490b6450106ec1be5bef344a63c4d40cacef508a2c74e34ecc22fe7  DNF_ForgedBlade/TTF/DNFForgedBlade-Bold.ttf
4589cb1a59bcbd669ad7ac0669827e5a4d411832048e9bdd9618c907c1a8d272  x10y12pxDenkiChipHangul/x10y12pxDenkiChipHangul.ttf
954e4cbdc8476561cdd042aa2f22d4fbfea57dbfc4beb45e307ac1c1c6938d5a  x12y12pxMaruMinyaHangul/x12y12pxMaruMinyaHangul.ttf
```

일부 테스트는 이 밖에 `../fonts/NEXON_Maplestory/TTF/Maplestory Light.ttf`(SHA-256 `6d51d8e576f77b01914095aa1f69f9d37c16d93fe940d748962867f218442ba9`)를 읽습니다.

### 디스크와 패치 생성

```bash
CUE="roms/Shiritsu Justice Gakuen - Nekketsu Seishun Nikki 2 (Japan).cue"

# 1. 번역을 해석해 대사 배치와 문자 코드를 확정
cargo run --release -- prepare-dialogue-inputs --cue "$CUE" --output-dir out/prepared-dialogue

# 2. 디스크 생성과 readback·EDC/ECC 검증
cargo run --release -- build-dialogue-disc \
  --cue "$CUE" \
  --prepared-dialogue out/prepared-dialogue \
  --input-policy complete-scope \
  --output-dir out/disc

# 3. xdelta 패치 생성
xdelta3 -e -9 -A -S none \
  -s "roms/Shiritsu Justice Gakuen - Nekketsu Seishun Nikki 2 (Japan).bin" \
  out/disc/justice-gakuen2-korean-development.bin \
  out/justice-gakuen2-kr.xdelta
```

`--spec`의 기본값은 `assets/build/development.json`입니다. 출력 디렉터리에 BIN, CUE와 빌드 manifest(JSON)가 생깁니다. 배포 패치 v1.0.0의 입력으로 빌드한 BIN은 663,216,960바이트, SHA-256 `a9f2bd73a9b6e0ff62feb043d8445a693a4fa9a80ffc29c273d4670bba09fddb`이며, 위 xdelta3 명령으로 만든 패치는 배포 파일(SHA-256 `0ddd4d98f4089b14f22fe628730b1c7f45b25cc898701da8bc2d1d44cc31d69c`)과 같습니다.

### 그 밖의 명령

화면별 그래픽 생성과 감사, 원본 디스크 조사 명령이 함께 들어 있습니다. 사용법은 `cargo run -- help`와 `cargo run -- help <명령>`으로 확인할 수 있습니다.

## 문제 제보

[Issues](https://github.com/mcpads/justice-gakuen2-kr-patch/issues)에 패치 버전, 에뮬레이터와 버전, 발생한 모드·날짜·장소, 직전 조작을 적어 주세요. 화면 문제가 있다면 스크린샷도 도움이 됩니다. 원본 또는 패치된 게임 이미지는 첨부하지 마세요.

## 제작 및 글꼴

- 한국어 패치: [mcpads](https://github.com/mcpads)
- 원작: CAPCOM
- 사용 글꼴: 갈무리, Neo둥근모, DenkiChip Hangul, MaruMinya Hangul, 달무리, 넥슨 메이플스토리·Lv.2 고딕·풋볼 고딕, 던파 연단된 칼날

게임과 각 글꼴의 권리는 해당 권리자에게 있습니다. 글꼴 파일과 패치 적용 도구는 이 배포에 포함하지 않습니다.

## 라이선스

이 저장소의 소스 코드는 [MIT License](LICENSE)로 제공합니다.
