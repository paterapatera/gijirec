# 要件定義書

## はじめに

gijirec Whisper Transcribe は、audio-capture が供給するミックス済み 16 kHz モノラル PCM をローカルで逐次文字起こしし、発言から数秒以内にタイムスタンプ付きテキストを下流へストリーミング供給する機能である。本 spec は製品ロードマップ上 audio-capture の直下に位置し、transcript-editor の入力源となる。利用者は会議中にその場で発言内容をテキストとして追跡でき、モデル初回取得後はインターネット接続なしでも文字起こしを継続できる。

## スコープ境界

- **対象範囲**: ミックス PCM チャンクの継続消費、ローカル逐次音声認識、低遅延テキストストリームの下流供給、ブロック単位の音声開始タイムスタンプ付与、音声認識モデルの初回取得と取得後のオフライン推論、アプリ終了時の推論完全停止、会議並行利用を想定したリソース負荷の抑制
- **対象外**: 手動編集 UI、部分ロック、Markdown 出力、クラウド音声認識、話者分離、Python ランタイムの要求、音声キャプチャそのもの（audio-capture）、転写テキストの永続保存（transcript-editor 以降）、Linux 対応
- **隣接システム・仕様への期待**: 上流 audio-capture は `docs/contracts/audio-capture-pcm.md` に定義された `PcmChunk`（16 kHz / 16 bit / モノラル / 100 ms チャンク、`timestamp_ms` 付き）をキャプチャ有効中に連続供給すること。本機能はキャプチャの開始・停止を所有せず、キャプチャ状態と協調して推論を開始・停止する。下流 transcript-editor は、本機能が供給するタイムスタンプ付きテキストブロックを順次受け取り、表示・編集できること。本機能は編集状態やロック情報を所有しない。音声データおよび転写テキストを外部ネットワークへ送信しない（モデル初回取得を除く）。

## 要件

### 要件 1: ミックス PCM の継続消費

**目的:** 文字起こし利用者として、キャプチャ中のミックス済み音声が途切れず文字起こし処理へ渡されるようにしたい。その結果、会議の進行に沿った連続的な転写が可能になる。

#### 受け入れ条件

1. While 上流 audio-capture がキャプチャを有効にしている, the gijirec Whisper Transcribe shall audio-capture が供給する正規化済み PCM チャンク（16 kHz / 16 bit / モノラル）を継続的に受け取り、文字起こし処理へ供給する。
2. While キャプチャが有効である, the gijirec Whisper Transcribe shall 供給された PCM チャンクの順序欠落があっても処理を異常終了させず、利用可能なチャンクで文字起こしを継続する。
3. While キャプチャが有効である, the gijirec Whisper Transcribe shall 受信した PCM を文字起こし以外の目的（外部送信・ファイル永続化等）に使用しない。
4. The gijirec Whisper Transcribe shall 音声キャプチャのデバイス取得・ミキシング・権限要求を所有しない。

### 要件 2: ローカル逐次文字起こし

**目的:** 会議利用者として、クラウドサービスや追加ランタイムに依存せず、取り込んだ音声をそのマシン上で逐次文字起こししたい。その結果、オフライン会議や機密会議でもローカル完結で転写できる。

#### 受け入れ条件

1. While キャプチャが有効であり音声認識モデルが利用可能である, the gijirec Whisper Transcribe shall 受け取ったミックス PCM をローカル上で逐次音声認識し、テキストへ変換する。
2. The gijirec Whisper Transcribe shall 音声認識のためにクラウド音声認識サービスへ音声データを送信しない。
3. The gijirec Whisper Transcribe shall 音声認識の実行に Python ランタイムのインストールまたは起動を要求しない。
4. The gijirec Whisper Transcribe shall 話者を識別・分離したラベル付きテキストを生成しない。

### 要件 3: 低遅延テキストストリーム

**目的:** 会議利用者として、発言内容が録音後に一括起こしされるのを待たず、その場でテキストとして追えるようにしたい。その結果、会議進行中に内容を確認・後段編集へ引き渡せる。

#### 受け入れ条件

1. While キャプチャが有効であり継続的な発話がある, the gijirec Whisper Transcribe shall 会議終了を待たずに、認識済みテキストを下流へ順次追加供給する。
2. When 連続する発話区間が 1 回の推論ウィンドウ（3〜5 秒相当の音声）に含まれる, the gijirec Whisper Transcribe shall 当該区間に対応するテキストブロックを、当該区間の終了から **5 秒以内** に下流へ供給する。
3. While キャプチャが有効である, the gijirec Whisper Transcribe shall 無音または認識不能と判断される区間に対して、意味のあるテキストブロックを追加しない。
4. The gijirec Whisper Transcribe shall 手動編集 UI、部分ロック、Markdown 出力機能を提供しない。
5. The gijirec Whisper Transcribe shall 既に下流へ供給したテキストブロックの内容を変更または撤回せず、追記のみで供給する。

### 要件 4: ブロック単位タイムスタンプ

**目的:** 議事録利用者として、表示・保存される各テキスト断片が会議のどの時点の発話に対応するか分かるようにしたい。その結果、後段のエディタや Markdown 出力で時系列構造を維持できる。

#### 受け入れ条件

1. When テキストブロックを下流へ供給する, the gijirec Whisper Transcribe shall 当該ブロックに対応する音声の開始時刻を、キャプチャ開始基準の経過ミリ秒として付与する。
2. The gijirec Whisper Transcribe shall 付与する開始タイムスタンプを、対応する入力 PCM チャンクの時刻情報および上流 audio-capture の時刻基準と整合させる。
3. When 複数のテキストブロックを順次供給する, the gijirec Whisper Transcribe shall 各ブロックに独立した開始タイムスタンプを付与する。
4. The gijirec Whisper Transcribe shall テキストブロックの手動修正・ロック状態を管理しない。

### 要件 5: 音声認識モデルの取得とオフライン運用

**目的:** 会議利用者として、初回のみモデルを取得し、以後はインターネットなしでも文字起こしを続けたい。その結果、ネットワーク不安定な環境でも安定して利用できる。

#### 受け入れ条件

1. When 音声認識モデルがローカルに存在しない状態で文字起こしが必要となる, the gijirec Whisper Transcribe shall モデル取得を開始し、利用者が進行状況を把握できる形で取得中であることを示す。
2. When 音声認識モデルの初回取得が正常に完了する, the gijirec Whisper Transcribe shall 以後インターネット接続なしで文字起こしを実行できる。
3. While モデルがローカルに存在しキャプチャが有効である, the gijirec Whisper Transcribe shall 文字起こし処理のためにインターネット接続を要求しない。
4. If モデル初回取得が失敗する, the gijirec Whisper Transcribe shall 文字起こしを開始せず、利用者が理解できる形で失敗理由と次に取れる行動（例: ネットワーク確認、再試行）を通知する。
5. If ローカル音声認識モデルが破損または読み込み不能である, the gijirec Whisper Transcribe shall 文字起こしを開始せず、利用者が理解できる形で失敗理由と次に取れる行動（例: モデル再取得）を通知する。

### 要件 6: 推論の開始・停止

**目的:** 会議利用者として、アプリの起動・終了操作に連動して文字起こし処理も自動的に開始・完全停止したい。その結果、会議前後の操作やリソース解放を意識せずに利用できる。

#### 受け入れ条件

1. When 上流 audio-capture がキャプチャを開始し、音声認識モデルが利用可能である, the gijirec Whisper Transcribe shall 文字起こし処理を開始する。
2. When ユーザーがアプリウィンドウを閉じる, the gijirec Whisper Transcribe shall 進行中の推論を完了または安全に中断し、推論に関連するプロセスおよびリソースを完全に解放する。
3. When ユーザーが OS 標準の終了操作を行う（Mac: Cmd+Q、Windows: Alt+F4 等）, the gijirec Whisper Transcribe shall ウィンドウを閉じた場合と同様に推論を完全停止する。
4. While キャプチャが停止している, the gijirec Whisper Transcribe shall 新規の音声認識推論を開始しない。
5. When 推論を停止する, the gijirec Whisper Transcribe shall バックグラウンドで推論処理が継続しない。

### 要件 7: 会議中のリソース負荷

**目的:** 会議利用者として、文字起こし処理が会議本体や他アプリのパフォーマンスを著しく損なわないようにしたい。その結果、長時間の Web 会議中もキャプチャと文字起こしを並行利用できる。

#### 受け入れ条件

1. While キャプチャと文字起こしが同時に有効である, the gijirec Whisper Transcribe shall 同一マシン上で Web 会議アプリと並行実行されても、Web 会議アプリ側で持続的な音声または映像の途切れ（gijirec の文字起こし停止後に改善するもの）を引き起こさない。
2. While キャプチャと文字起こしが同時に有効である, the gijirec Whisper Transcribe shall 文字起こし処理に起因する CPU・メモリ使用量を、設計フェーズで定義する非機能テスト計画の合格基準以内に抑える。

### 要件 8: 障害・エラー時の挙動

**目的:** 会議利用者として、文字起こしが停止または劣化した場合でも原因と次の行動が分かるようにしたい。その結果、設定変更や再起動で回復できる。

#### 受け入れ条件

1. If ローカル音声認識処理が回復不能なエラーで停止する, the gijirec Whisper Transcribe shall 新規テキストブロックの供給を停止し、利用者が理解できる形で障害を通知する。
2. When 障害またはエラーを通知する, the gijirec Whisper Transcribe shall 技術的な内部エラーコードだけでなく、利用者が次に取れる行動（例: アプリ再起動、モデル再取得の確認）を示す。
3. If 上流 audio-capture がエラー状態へ遷移する, the gijirec Whisper Transcribe shall 新規 PCM の処理を停止し、キャプチャ復帰後に文字起こしを再開できる。
4. The gijirec Whisper Transcribe shall エラー通知およびログに、転写テキスト全文や PCM 生データを含めない。

### 要件 9: プライバシーとローカル完結

**目的:** 会議利用者として、会議音声と転写内容が意図せず外部へ送信されないようにしたい。その結果、機密会議でもローカル処理の範囲で安心して利用できる。

#### 受け入れ条件

1. While 文字起こしが有効である, the gijirec Whisper Transcribe shall 音声データおよび認識済みテキストを、音声認識モデルの初回取得を除き、外部ネットワークへ送信しない。
2. The gijirec Whisper Transcribe shall 転写テキストをユーザー明示操作なしに永続ファイルへ保存しない（下流処理およびメモリ上の一時保持のみ。保持期間の詳細は設計で定義する）。
3. The gijirec Whisper Transcribe shall ユーザー認証・認可機能を提供しない（ローカルデスクトップアプリの単一利用者前提。多ユーザー認可は対象外）。
4. The gijirec Whisper Transcribe shall 取得・生成した転写テキストを、本機能の下流供給以外の目的で第三者サービスへ送信しない。
