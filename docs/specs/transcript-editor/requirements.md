# 要件定義書

## はじめに

gijirec Transcript Editor は、会議中にユーザーが手動で議事録を取りながら、上流 whisper-transcribe が供給するリアルタイム文字起こしをその場で追跡・編集できる編集体験を提供する機能である。現状はストリーミングテキストの手動修正が自動更新に上書きされ、レイアウトシフトや点滅で編集できず、成果物を Markdown として残せない。本機能では部分ロック付きの二重エディタ（手動議事録＋ AI 転写表示）、安定したストリーミング表示、保存先設定の永続化、および `handwriting.md` / `ai-transcription.md`（オプションで `ai-transcription.jsonl`）のファイル出力を実現する。手動議事録は内容は正確だが荒く、AI 議事録は誤字が多いが細かいという特性を前提とし、ユーザーが両ファイルを手動で組み合わせて議事録を清書する想定である（自動マージは対象外）。

## スコープ境界

- **対象範囲**: 上流転写ブロックのリアルタイム表示、手動議事録エディタ、選択／入力箇所の部分ロックによる AI 上書き防止、ストリーミング追加時のレイアウト安定、保存先ディレクトリの設定と永続化、日時ベースのサブディレクトリ構成での `handwriting.md` / `ai-transcription.md` 出力、オプションの `ai-transcription.jsonl`（タイムスタンプ付き）出力、保存・設定エラー時の利用者通知
- **対象外**: 音声キャプチャ、Whisper 推論本体、仮想オーディオデバイス、クラウド同期、清書の自動マージ（ユーザーが手動で行う）、話者分離、Linux 対応、ユーザー認証・認可
- **隣接システム・仕様への期待**: 上流 whisper-transcribe は `docs/contracts/whisper-transcribe-blocks.md` に定義されたタイムスタンプ付きテキストブロックを追記のみで供給すること。本機能はキャプチャの開始・停止や推論処理を所有せず、供給されたブロックを表示・編集・保存する。上流が既に供給したブロックの内容を変更・撤回しない前提で動作する。本機能は編集ロック状態や手動議事録の内容を上流へ返送しない。音声データおよび転写テキストを外部ネットワークへ送信しない。
- **機微データ**: 手動議事録および AI 転写内容はローカル完結の機微データとして扱い、利用者の明示的な保存操作以外では永続化しない（`docs/steering/security.md` 準拠）。

## 要件

### 要件 1: 上流転写ブロックのリアルタイム表示

**目的:** 会議利用者として、AI 文字起こしの結果が会議の進行に沿って画面に流れるようにしたい。その結果、発言内容をその場で追跡し、必要な箇所をすぐ確認できる。

#### 受け入れ条件

1. While 上流 whisper-transcribe がキャプチャ連動で文字起こしを行っている, the gijirec Transcript Editor shall 供給されるテキストブロックを受信し、AI 転写表示領域へ順次追記表示する。
2. When 新しいテキストブロックが供給される, the gijirec Transcript Editor shall 既に表示済みのブロックの内容を変更または削除せず、追記のみで表示を更新する。
3. While 文字起こしが有効である, the gijirec Transcript Editor shall 利用者が手動議事録の編集を継続できる状態を維持する。
4. When 上流 whisper-transcribe が新規ブロックの供給を停止する, the gijirec Transcript Editor shall それまでに受信した内容を保持し、利用者が閲覧・編集・保存できる状態を維持する。
5. The gijirec Transcript Editor shall 音声キャプチャ、Whisper 推論、モデル取得処理を所有しない。
6. While 上流 whisper-transcribe からタイムスタンプ付きテキストブロックを受信している, the gijirec Transcript Editor shall 各ブロックの開始タイムスタンプ（キャプチャ開始基準の経過ミリ秒）と AI 転写表示領域内の対応内容との関連を、部分ロックおよび手動修正後も維持する（表示上タイムスタンプを示さなくてもよい）。

### 要件 2: 手動議事録エディタ

**目的:** 会議利用者として、会議の要点や補足を自分の言葉でその場に記録したい。その結果、AI 転写と並行して正確だが荒い手動メモを残せる。

#### 受け入れ条件

1. The gijirec Transcript Editor shall 手動議事録用の編集領域を AI 転写表示領域とは独立して提供する。
2. When 利用者が手動議事録領域へ文字を入力する, the gijirec Transcript Editor shall 入力内容を即時に反映し、編集操作を継続できる。
3. While 会議が進行している, the gijirec Transcript Editor shall 手動議事録の編集と AI 転写の追記表示を同時に利用可能にする。
4. The gijirec Transcript Editor shall 手動議事録の内容を、利用者の明示的な保存操作なしにディスクへ書き出さない。

### 要件 3: 部分ロックによる AI 上書き防止

**目的:** 会議利用者として、自分が修正または注記した箇所が、後から届く AI 転写の自動更新で消えないようにしたい。その結果、リアルタイム文字起こしを受けながら安心して手直しできる。

#### 受け入れ条件

1. When 利用者が AI 転写表示領域の一部を選択する, the gijirec Transcript Editor shall 当該範囲をロックし、以後の上流ブロック追記による当該範囲の内容変更を行わない。
2. When 利用者が AI 転写表示領域へ直接文字を入力または修正する, the gijirec Transcript Editor shall 当該編集箇所をロックし、以後の上流ブロック追記による当該箇所の上書きを行わない。
3. While 一部がロックされている, the gijirec Transcript Editor shall ロックされていない範囲へは引き続き上流ブロックを追記表示できる。
4. When 利用者がロック済み範囲を再度編集する, the gijirec Transcript Editor shall 利用者の手動編集内容を優先し、ロック状態を維持する。
5. The gijirec Transcript Editor shall ロック状態や手動修正内容を上流 whisper-transcribe へ送信しない。

### 要件 4: ストリーミング表示の安定性

**目的:** 会議利用者として、文字起こしテキストが追加されるたびに画面が激しく動かず、視認と編集が途切れないようにしたい。その結果、長時間の会議でも快適に追記・手直しできる。

#### 受け入れ条件

1. When 新しいテキストブロックが AI 転写表示領域へ追記される, the gijirec Transcript Editor shall 追記のみの更新によって、利用者が現在のスクロール位置を維持したまま読んでいる既存行が意図せず大きくずれない（激しいレイアウトシフトを起こさない）。
2. When 新しいテキストブロックが追記される, the gijirec Transcript Editor shall 表示済みの同一テキストが一度消えてから同一内容として再表示されるような更新を行わない（点滅・ちらつきを起こさない）。
3. While 利用者が手動議事録または AI 転写表示領域を編集中である, the gijirec Transcript Editor shall 新規ブロック追記によって編集中のカーソル位置や選択範囲が意図せず失われない。
4. While 連続して複数のテキストブロックが短時間に供給される, the gijirec Transcript Editor shall 追記表示を継続し、手動議事録または AI 転写表示領域での文字入力・選択操作を中断させない。

### 要件 5: 保存先ディレクトリの設定と永続化

**目的:** 会議利用者として、議事録ファイルの保存場所を自分の環境に合わせて指定し、次回起動時も同じ設定を使いたい。その結果、毎回保存先を選び直す手間がなくなる。

#### 受け入れ条件

1. The gijirec Transcript Editor shall 利用者がファイル保存の基点となるディレクトリ（保存先ディレクトリ）を設定できる。
2. When 利用者が保存先ディレクトリを変更する, the gijirec Transcript Editor shall 変更後の保存先ディレクトリをアプリ再起動後も保持する。
3. When アプリを再起動する, the gijirec Transcript Editor shall 前回設定した保存先ディレクトリを復元する。
4. If 保存先ディレクトリが存在しない、または書き込み権限がない, the gijirec Transcript Editor shall 保存を完了せず、利用者が理解できる形で理由と次に取れる行動（例: 別のディレクトリを選択）を通知する。
5. If 保存先ディレクトリが未設定である, the gijirec Transcript Editor shall 保存を完了せず、保存先ディレクトリの設定を促す通知を行う。

### 要件 6: 日時ベースのサブディレクトリ構成

**目的:** 会議利用者として、保存した議事録が日付ごとに整理され、同じ日に複数回保存しても上書きされないようにしたい。その結果、過去の会議記録を探しやすくなる。

#### 受け入れ条件

1. When 利用者が保存操作を実行する, the gijirec Transcript Editor shall 保存先ディレクトリを基点として `{YYYY}/{MM}/{DD}/{hh}_{mm}_{ss}/` 形式のサブディレクトリを作成し、当該保存の成果物をその中へ出力する（`YYYY`・`MM`・`DD`・`hh`・`mm`・`ss` は保存操作実行時刻の日本標準時（JST, UTC+9）に基づく）。
2. When 同一秒内に複数回保存操作が実行される, the gijirec Transcript Editor shall 各保存の成果物が互いに上書きされないよう、一意なサブディレクトリまたは同等の分離手段を用いる。
3. If サブディレクトリの作成に失敗する, the gijirec Transcript Editor shall ファイル出力を完了せず、利用者が理解できる形で失敗理由と次に取れる行動を通知する。

### 要件 7: Markdown ファイル出力

**目的:** 会議利用者として、手動議事録と AI 文字起こしを別ファイルとして Markdown で残し、後から手動で清書に使いたい。その結果、会議終了後に外部エディタでも編集・共有できる。

#### 受け入れ条件

1. When 利用者が保存操作を実行する, the gijirec Transcript Editor shall 保存開始時点の手動議事録の内容を `handwriting.md` として、要件 6 で作成したサブディレクトリへ出力する。
2. When 利用者が保存操作を実行する, the gijirec Transcript Editor shall 保存開始時点の AI 転写表示領域の内容（ロック済み手動修正を含む）を `ai-transcription.md` として、同一サブディレクトリへ出力する。
3. When 利用者が保存操作を実行する, the gijirec Transcript Editor shall 保存処理中に供給された上流テキストブロックを、当該保存で出力する `handwriting.md`・`ai-transcription.md` および有効時の `ai-transcription.jsonl` に含めない。
4. When 利用者が保存操作を実行する, the gijirec Transcript Editor shall 上流 whisper-transcribe の文字起こしを停止せず、保存完了後も AI 転写の追記表示を継続する。
5. The gijirec Transcript Editor shall `ai-transcription.md` にタイムスタンプ情報を含めず、プレーンテキストのみを出力する。
6. When 保存操作が正常に完了する, the gijirec Transcript Editor shall 出力したファイルパスまたは保存先を利用者が確認できる形で示す。
7. If ファイル書き込みが失敗する, the gijirec Transcript Editor shall 部分的な成功と失敗を区別して通知し、利用者が再試行または保存先変更を判断できる情報を提供する。
8. The gijirec Transcript Editor shall 手動議事録と AI 転写の内容を自動的にマージした単一ファイルを生成しない。

### 要件 8: オプション JSONL 出力（タイムスタンプ付き）

**目的:** 会議利用者として、必要に応じて AI 転写の時系列構造をタイムスタンプ付きで残したい。その結果、後段の手動清書や分析で発話の順序と時刻を参照できる。

#### 受け入れ条件

1. Where 利用者がタイムスタンプ付き出力を有効にしている, the gijirec Transcript Editor shall 保存開始時点の AI 転写ブロック構造を `ai-transcription.jsonl` として、要件 6 で作成したサブディレクトリへ出力する。
2. Where `ai-transcription.jsonl` を出力する, the gijirec Transcript Editor shall 各レコードに、上流 whisper-transcribe が供給したブロックに対応する開始タイムスタンプ（キャプチャ開始基準の経過ミリ秒）を含める。
3. Where 利用者がタイムスタンプ付き出力を無効にしている, the gijirec Transcript Editor shall `ai-transcription.jsonl` を生成しない。
4. The gijirec Transcript Editor shall タイムスタンプ付き出力の有効／無効設定を、アプリ再起動後も保持する。
5. The gijirec Transcript Editor shall 利用者がタイムスタンプ付き JSONL 出力の有効／無効を切り替えられる手段を提供する。

### 要件 9: 保存・設定エラー時の挙動

**目的:** 会議利用者として、保存や設定変更が失敗した場合でも原因と次の行動が分かるようにしたい。その結果、データ損失を避けつつ回復できる。

#### 受け入れ条件

1. When 保存または設定変更でエラーを通知する, the gijirec Transcript Editor shall 技術的な内部エラーコードだけでなく、利用者が次に取れる行動（例: 保存先の変更、ディスク容量の確認、再試行）を示す。
2. If 保存操作が失敗する, the gijirec Transcript Editor shall 画面上の手動議事録および AI 転写表示内容を失わない。
3. If 上流 whisper-transcribe がエラー状態へ遷移する, the gijirec Transcript Editor shall それまでに表示・編集した内容を保持し、利用者が保存操作を実行できる。
4. The gijirec Transcript Editor shall エラー通知およびログに、転写テキスト全文や手動議事録の全文を含めない。

### 要件 10: プライバシーとローカル完結

**目的:** 会議利用者として、議事録の内容が意図せず外部へ送信されないようにしたい。その結果、機密会議でもローカル処理の範囲で安心して編集・保存できる。

#### 受け入れ条件

1. The gijirec Transcript Editor shall 手動議事録および AI 転写内容を外部ネットワークへ送信しない。
2. The gijirec Transcript Editor shall 利用者の明示的な保存操作以外で、議事録内容をディスクへ書き出さない。
3. The gijirec Transcript Editor shall クラウド同期機能を提供しない。
4. The gijirec Transcript Editor shall ユーザー認証・認可機能を提供しない（ローカルデスクトップアプリの単一利用者前提）。
