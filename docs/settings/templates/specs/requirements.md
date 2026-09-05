# 要件定義書

## はじめに
{{INTRODUCTION}}

<!-- スコープの誤解が生じやすい場合、または隣接システム・仕様に触れる場合に記載 -->
## スコープ境界（任意）
- **対象範囲**: {{IN_SCOPE_BEHAVIORS}}
- **対象外**: {{OUT_OF_SCOPE_BEHAVIORS}}
- **隣接システム・仕様への期待**: {{ADJACENT_SYSTEM_OR_SPEC_EXPECTATIONS}}

## 要件

### 要件 1: {{REQUIREMENT_AREA_1}}
<!-- 要件見出しは先頭に数値 ID のみを付ける（例: 「要件 1: ...」「1. 概要」「2 機能: ...」）。「要件 A」のような英字 ID は不可 -->
**目的:** {{ROLE}}として、{{CAPABILITY}}したい。その結果、{{BENEFIT}}となる。

#### 受け入れ条件
<!-- 受け入れ条件は spec.json で指定した言語で記述する。EARS のトリガー語句は英語のまま（When, If, While, Where, The [system] shall） -->
1. When [イベント], the [システム名] shall [応答/動作]
2. If [トリガー], then the [システム名] shall [応答/動作]
3. While [前提条件], the [システム名] shall [応答/動作]
4. Where [機能が含まれる場合], the [システム名] shall [応答/動作]
5. The [システム名] shall [応答/動作]

### 要件 2: {{REQUIREMENT_AREA_2}}
**目的:** {{ROLE}}として、{{CAPABILITY}}したい。その結果、{{BENEFIT}}となる。

#### 受け入れ条件
1. When [イベント], the [システム名] shall [応答/動作]
2. When [イベント] and [条件], the [システム名] shall [応答/動作]

<!-- 追加の要件も同じ形式で記述 -->
