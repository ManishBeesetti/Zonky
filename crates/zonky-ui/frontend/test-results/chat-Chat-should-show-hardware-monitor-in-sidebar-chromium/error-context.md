# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: chat.spec.js >> Chat >> should show hardware monitor in sidebar
- Location: tests/chat.spec.js:124:3

# Error details

```
Error: expect(locator).toBeVisible() failed

Locator: locator('main aside').getByText('Throughput')
Expected: visible
Timeout: 5000ms
Error: element(s) not found

Call log:
  - Expect "toBeVisible" with timeout 5000ms
  - waiting for locator('main aside').getByText('Throughput')

```

# Page snapshot

```yaml
- generic [ref=e2]:
  - navigation [ref=e3]:
    - generic [ref=e4]: ZONKY
    - generic [ref=e5]:
      - button "dashboard Dashboard" [ref=e6] [cursor=pointer]:
        - generic [ref=e7]: dashboard
        - generic [ref=e8]: Dashboard
      - button "library_books Library" [ref=e9] [cursor=pointer]:
        - generic [ref=e10]: library_books
        - generic [ref=e11]: Library
      - button "extension Models" [ref=e12] [cursor=pointer]:
        - generic [ref=e13]: extension
        - generic [ref=e14]: Models
      - button "cloud_download Downloads" [ref=e15] [cursor=pointer]:
        - generic [ref=e16]: cloud_download
        - generic [ref=e17]: Downloads
      - button "chat_bubble Chat" [active] [ref=e18] [cursor=pointer]:
        - generic [ref=e19]: chat_bubble
        - generic [ref=e20]: Chat
    - button "settings Settings" [ref=e21] [cursor=pointer]:
      - generic [ref=e22]: settings
      - generic [ref=e23]: Settings
  - banner [ref=e24]:
    - generic [ref=e25]:
      - generic [ref=e26]: search
      - generic [ref=e27]: System Search
    - generic [ref=e28]:
      - generic [ref=e29]:
        - generic [ref=e30]: "Active Model:"
        - generic [ref=e31]: test-model-1
      - generic [ref=e34]: System Ready
      - button "cloud_download" [ref=e35] [cursor=pointer]:
        - generic [ref=e36]: cloud_download
      - button "more_vert" [ref=e37] [cursor=pointer]:
        - generic [ref=e38]: more_vert
  - main [ref=e40]:
    - generic [ref=e42]:
      - generic [ref=e43]:
        - generic [ref=e44]:
          - generic [ref=e45]: Model
          - combobox [ref=e46] [cursor=pointer]:
            - option "test-model-1" [selected]
          - button "refresh Refresh" [ref=e47] [cursor=pointer]:
            - generic [ref=e48]: refresh
            - text: Refresh
        - generic [ref=e50]:
          - generic [ref=e52]: smart_toy
          - heading "Start Inference" [level=2] [ref=e53]
          - paragraph [ref=e54]: Load a model from the Library, then begin a conversation. Zonky will run inference locally on your hardware.
          - generic [ref=e56]:
            - generic [ref=e57]: info
            - paragraph [ref=e58]: Select a model above and start chatting.
        - generic [ref=e61]:
          - textbox "Enter inference instructions..." [ref=e62]
          - generic [ref=e63]:
            - button "delete_sweep Clear Chat" [ref=e65] [cursor=pointer]:
              - generic [ref=e66]: delete_sweep
              - generic [ref=e67]: Clear Chat
            - button "Run Inference bolt" [disabled] [ref=e68]:
              - generic [ref=e69]: Run Inference
              - generic [ref=e70]: bolt
      - complementary [ref=e71]:
        - generic [ref=e72]:
          - generic [ref=e73]:
            - heading "Inference Config" [level=3] [ref=e74]
            - generic [ref=e75]: tune
          - generic [ref=e76]:
            - generic [ref=e77]:
              - generic [ref=e78]:
                - generic [ref=e79]: Temperature
                - generic [ref=e80]: "0.75"
              - slider "Temperature" [ref=e81] [cursor=pointer]: "0.75"
              - paragraph [ref=e82]: "Controls randomness: Lower is more deterministic."
            - generic [ref=e83]:
              - generic [ref=e84]:
                - generic [ref=e85]: Top-P
                - generic [ref=e86]: "0.90"
              - slider "Top-P" [ref=e87] [cursor=pointer]: "0.9"
              - paragraph [ref=e88]: Nucleus sampling threshold for token vocabulary.
            - generic [ref=e89]:
              - generic [ref=e90]:
                - generic [ref=e91]: Repeat Penalty
                - generic [ref=e92]: "1.10"
              - slider "Repeat Penalty" [ref=e93] [cursor=pointer]: "1.1"
          - generic [ref=e94]:
            - heading "Hardware Monitor" [level=3] [ref=e95]
            - generic [ref=e96]:
              - generic [ref=e99]:
                - generic [ref=e100]: GPU Memory
                - generic [ref=e101]: 6.0 / 96.0 GB
              - generic [ref=e104]:
                - generic [ref=e105]: Neural Engine
                - generic [ref=e106]: 6% Utilization
          - generic [ref=e108]:
            - generic [ref=e109]: info
            - paragraph [ref=e110]: Context window currently set to 8,192 tokens.
  - contentinfo [ref=e111]:
    - generic [ref=e112]:
      - generic [ref=e113]: System Ready
      - generic [ref=e114]: "|"
      - generic [ref=e117]: test-model-1
    - generic [ref=e119]:
      - generic [ref=e120]: memory
      - generic [ref=e121]: "VRAM: 6.0GB / 96.0GB"
```

# Test source

```ts
  32  |   });
  33  | 
  34  |   test("should have chat input with placeholder", async ({ page }) => {
  35  |     await page.goto("/");
  36  |     await page.locator("nav button", { hasText: "Chat" }).click();
  37  | 
  38  |     const textarea = page.locator(
  39  |       'main textarea[placeholder="Enter inference instructions..."]'
  40  |     );
  41  |     await expect(textarea).toBeVisible();
  42  |   });
  43  | 
  44  |   test("should have Run Inference button", async ({ page }) => {
  45  |     await page.goto("/");
  46  |     await page.locator("nav button", { hasText: "Chat" }).click();
  47  | 
  48  |     await expect(
  49  |       page.locator("main button", { hasText: "Run Inference" })
  50  |     ).toBeVisible();
  51  |   });
  52  | 
  53  |   test("should disable Run Inference when input is empty", async ({
  54  |     page,
  55  |   }) => {
  56  |     await page.goto("/");
  57  |     await page.locator("nav button", { hasText: "Chat" }).click();
  58  | 
  59  |     const runButton = page.locator("main button", { hasText: "Run Inference" });
  60  |     await expect(runButton).toBeDisabled();
  61  |   });
  62  | 
  63  |   test("should send message and display response", async ({ page }) => {
  64  |     await page.goto("/");
  65  |     await page.locator("nav button", { hasText: "Chat" }).click();
  66  | 
  67  |     // Type a message
  68  |     const textarea = page.locator(
  69  |       'main textarea[placeholder="Enter inference instructions..."]'
  70  |     );
  71  |     await textarea.fill("Hello, test message!");
  72  | 
  73  |     // Click Run Inference
  74  |     await page.locator("main button", { hasText: "Run Inference" }).click();
  75  | 
  76  |     // User message should appear
  77  |     await expect(page.locator("main").getByText("Hello, test message!")).toBeVisible();
  78  | 
  79  |     // Assistant response from mock should appear
  80  |     await expect(
  81  |       page.locator("main").getByText(
  82  |         "Hello! I'm a test response from the mocked inference backend."
  83  |       )
  84  |     ).toBeVisible({ timeout: 5000 });
  85  |   });
  86  | 
  87  |   test("should clear chat when Clear Chat button is clicked", async ({
  88  |     page,
  89  |   }) => {
  90  |     await page.goto("/");
  91  |     await page.locator("nav button", { hasText: "Chat" }).click();
  92  | 
  93  |     // Send a message first
  94  |     const textarea = page.locator(
  95  |       'main textarea[placeholder="Enter inference instructions..."]'
  96  |     );
  97  |     await textarea.fill("Test message");
  98  |     await page.locator("main button", { hasText: "Run Inference" }).click();
  99  | 
  100 |     // Wait for response
  101 |     await expect(
  102 |       page.locator("main").getByText(
  103 |         "Hello! I'm a test response from the mocked inference backend."
  104 |       )
  105 |     ).toBeVisible({ timeout: 5000 });
  106 | 
  107 |     // Click Clear Chat
  108 |     await page.locator("main button", { hasText: "Clear Chat" }).click();
  109 | 
  110 |     // Messages should be gone, empty state should return
  111 |     await expect(page.getByRole("heading", { name: "Start Inference" })).toBeVisible();
  112 |   });
  113 | 
  114 |   test("should show inference config panel", async ({ page }) => {
  115 |     await page.goto("/");
  116 |     await page.locator("nav button", { hasText: "Chat" }).click();
  117 | 
  118 |     await expect(page.locator("main").getByText("Inference Config")).toBeVisible();
  119 |     await expect(page.locator("main").getByText("Temperature")).toBeVisible();
  120 |     await expect(page.locator("main").getByText("Top-P")).toBeVisible();
  121 |     await expect(page.locator("main").getByText("Repeat Penalty")).toBeVisible();
  122 |   });
  123 | 
  124 |   test("should show hardware monitor in sidebar", async ({ page }) => {
  125 |     await page.goto("/");
  126 |     await page.locator("nav button", { hasText: "Chat" }).click();
  127 | 
  128 |     // Scope to the chat aside panel to avoid matching Dashboard
  129 |     const chatAside = page.locator("main aside");
  130 |     await expect(chatAside.getByText("Hardware Monitor")).toBeVisible();
  131 |     await expect(chatAside.getByText("GPU Memory")).toBeVisible();
> 132 |     await expect(chatAside.getByText("Throughput")).toBeVisible();
      |                                                     ^ Error: expect(locator).toBeVisible() failed
  133 |   });
  134 | 
  135 |   test("should show no models loaded hint when none loaded", async ({
  136 |     page,
  137 |   }) => {
  138 |     test.skip();
  139 |   });
  140 | });
  141 | 
```