
Branch tags: #branch/main #branch/release-microsoft-store #branch/integration-cuda #branch/integration-combined

Here are pages:
[Getting Started \| Deepgram's Docs](https://developers.deepgram.com/docs/live-streaming-audio)
[Live Audio \| Deepgram's Docs](https://developers.deepgram.com/reference/speech-to-text/listen-streaming)

[Live Streaming Starter Kit \| Deepgram's Docs](https://developers.deepgram.com/docs/getting-started-with-the-streaming-test-suite)

[STT Troubleshooting WebSocket, NET, and DATA Errors \| Deepgram's Docs](https://developers.deepgram.com/docs/stt-troubleshooting-websocket-data-and-net-errors)

[Errors \| Deepgram's Docs](https://developers.deepgram.com/docs/errors)

[Close Stream \| Deepgram's Docs](https://developers.deepgram.com/docs/close-stream)

[Finalize \| Deepgram's Docs](https://developers.deepgram.com/docs/finalize)

[Audio Keep Alive \| Deepgram's Docs](https://developers.deepgram.com/docs/audio-keep-alive)

[Determining Your Audio Format for Live Streaming Audio \| Deepgram's Docs](https://developers.deepgram.com/docs/determining-your-audio-format-for-live-streaming-audio)
[Using Lower-Level Websockets with the Streaming API \| Deepgram's Docs](https://developers.deepgram.com/docs/lower-level-websockets)

[Recovering From Connection Errors & Timeouts When Live Streaming \| Deepgram's Docs](https://developers.deepgram.com/docs/recovering-from-connection-errors-and-timeouts-when-live-streaming-audio)

Example repo: this is official rust example repositorium cloned from DeepGram:
C:\Code\experiments\rust-text-to-speech

---

# Text:
---




***

title: Recovering From Connection Errors & Timeouts When Live Streaming
subtitle: >-
Learn how to implement real-time, live streaming transcription solutions for
long-running audio streams.
slug: docs/recovering-from-connection-errors-and-timeouts-when-live-streaming-audio
-----------------------------------------------------------------------------------

Deepgram's API allows you to live stream audio for real-time transcription. Our live streaming service can be used with WebSocket streams. The longer a WebSocket stream persists, the more chances there are for transient network or service issues to cause a break in the connection. We recommend that you be prepared to gracefully recover from streaming errors, especially if you plan to live-stream audio for long periods of time (for example, if you are getting live transcription of a day-long virtual conference).

Implementing solutions that correctly handle disrupted connections can be challenging. In this guide, we recommend some solutions to the most common issues developers face when implementing real-time transcription with long-running live audio streams.

## Before You Begin

Before you begin, make sure you:

* have basic familiarity with Deepgram's API, specifically its [Transcribe Streaming Audio endpoint](/reference/speech-to-text/listen-streaming).
* understand how to make WebSocket requests and receive API responses.

## Main Issues

When you use Deepgram's API for real-time transcription with long-running live audio streams, you should be aware of some challenges you could encounter.

### Disrupted Connections

While Deepgram makes every effort to preserve streams, it's always possible that the connection could be disrupted. This may be due to internal factors or external ones, including bandwidth limitations and network failures.

In these cases, your application must initialize a new WebSocket connection and start a new streaming session. Once the new WebSocket connection is accepted and you receive the message indicating the connection is open, your application can begin streaming audio to it. You must stream audio to the new connection within 10 seconds of opening, or the connection will close due to lack of data.

## Data Loss

If you must reconnect to the Deepgram API for any reason, you could encounter loss of data while you are reconnecting since audio data will still be produced, but will not be transferred to our API during this period.

To avoid losing the produced audio while you are recovering the connection, you should have a strategy in place. We recommend that your application stores the audio data in a buffer until it can re-establish the connection to our API and then sends the data for delayed transcription. Because Deepgram allows audio to be streamed at a maximum of 1.25x realtime, if you send a large buffer of audio, the stream may wind up being significantly delayed.

## Corrupt Timestamps

Deepgram returns transcripts that include timestamps for every transcribed word. Timestamps correspond to the moments when the words are spoken within the audio. Every time you reconnect to our API, you create a new connection, so the timestamps on your audio begin from 00:00:00 again.

Because of this, when you restart an interrupted streaming session, you'll need to be sure to realign the timestamps to the audio stream. We recommend that your application maintains a starting timestamp to offset all returned timestamps. When you process a timestamp returned from Deepgram, add your maintained starting timestamp to the returned timestamp to ensure that it is offset by the correct amount of time.

***




***

title: Using Lower-Level Websockets with the Streaming API
subtitle: >-
Learn how to implement using lower-level websockets with Deepgram's Streaming
API.
slug: docs/lower-level-websockets
---------------------------------

The [Deepgram's Streaming API](/reference/speech-to-text/listen-streaming) unlocks many use cases ranging from captioning to notetaking and much more. If you aren't able to use our Deepgram SDKs for your Streaming needs, this guide will provide a Reference Implementation for you.

<Info>
  Most users will not need this Reference Implementation because Deepgram provides [SDKs](/home) that already implement the Streaming API. This is an **optional** guide to help individuals interested in building and maintaining their own SDK specific to the Deepgram Streaming API.
</Info>

For additional reference see our Deepgram SDKs which include the Websocket-based Streaming API:

* [Javascript SDK](/home)
* [Python SDK](/home)
* [.NET SDK](/home)
* [Go SDK](/home)

## Using a Deepgram SDK vs Building Your Own SDK

The Deepgram SDKs should address most needs; however, if you find limitations or issues in any of the above SDKs, we encourage you to report issues, bugs, or ideas for new features in the open source repositories. Our SDK projects are open to code contributions as well.

If you still need to implement your own SDK, this guide will enable you to do that.

## Prerequisites

It is highly recommended that you familiarize yourself with the WebSocket protocol defined by [RFC-6455](https://datatracker.ietf.org/doc/html/rfc6455). If you are still getting familiar with what an [IETF RFC](https://www.ietf.org/standards/rfcs/) is, they are very detailed specifications on how something works and behaves. In this case, [RFC-6455](https://datatracker.ietf.org/doc/html/rfc6455) describes how to implement WebSockets. You will need to understand this document to understand how to interact with the Deepgram Streaming API.

Once you understand the WebSocket protocol, it's recommended to understand the capabilities of your WebSocket protocol library available in the language you chose to implement your SDK in.

Refer to the language specific implementations for [RFC-6455](https://datatracker.ietf.org/doc/html/rfc6455) (i.e. the WebSocket protocol):

* [JavaScript](https://developer.mozilla.org/en-US/docs/Web/API/WebSocket)
* [Python](https://github.com/python-websockets/websockets)
* [GOrilla](https://github.com/gorilla/websocket) or [Go Networking](https://cs.opensource.google/go/x/net)
* [C# .NET](https://learn.microsoft.com/en-us/aspnet/core/fundamentals/websockets?view=aspnetcore-8.0)

These are just some of the available implementations in those languages. They are just the ones that are very popular in those language-specific communities.

Additionally, you will need to understand applications that are [multi-threaded](https://en.wikipedia.org/wiki/Multithreading_\(computer_architecture\)), [access the internet](https://en.wikipedia.org/wiki/Computer_network_programming), and do so [securely via TLS](https://en.wikipedia.org/wiki/Secure_Sockets_Layer). These are going to be essential components to building your SDK.

## Deepgram Streaming API

The goal of your SDK should minimally be:

* **Manage the Connection Lifecycle**: Implement robust connection management to handle opening, error handling, message sending, receiving, and closing of the WebSocket connection.
* **Concurrency and Threading**: Depending on the SDK's target language, manage concurrency appropriately to handle the asynchronous nature of WebSocket communication without blocking the main thread.
* **Error Handling and Reconnection**: Implement error handling and automatic reconnection logic. Transient network issues should not result in lost data or service interruptions.
* **Implement KeepAlives**: Deepgram's API may require keepalive messages to maintain the connection. Implement a mechanism to send periodic pings or other suitable messages to prevent timeouts.

## High-Level Pseudo-Code for Deepgram Streaming API

It's essential that you encapsulate your WebSocket connection in a class or similar representation. This will reduce undesired, highly coupled WebSocket code with your application's code. In the industry, this has often been referred to as minimizing ["Spaghetti code"](https://en.wikipedia.org/wiki/Spaghetti_code). If you have WebSocket code or you need to import the above WebSocket libraries into your `func main()`, this is undesirable unless your application is trivially small.

To implement the WebSocket Client correctly, you must implement based on the WebSocket protocol defined in [RFC-6455](https://datatracker.ietf.org/doc/html/rfc6455). Please refer to section [4.1 Client Requirements](https://datatracker.ietf.org/doc/html/rfc6455#section-4.1) in RFC-6455.

You want first to declare a WebSocket class of some sort specific to your implementation language:

<CodeGroup>
  ```text Text
  // This class could simply be called WebSocketClient
  // However, since this is specifically for Deepgram, it could be called DeepgramClient
  class WebSocketClient {
    private url: String
    private apiKey: String
    private websocket: WebSocket
    
    // other class properties
    
    // other class methods
  }
  ```
</CodeGroup>

**NOTE:** Depending on the programming language of choice, you might either need to implement `async`/`await` and `threaded` classes to support both threading models. These concepts occur in languages like Javascript, Python, and others. You can implement one or both based on your user's needs.

You will then need to implement the following class methods.

### Function: Connect

```perl
class WebSocketClient {
  ...
  function Connect() {
    // Implement the websocket connection here 
  }
  ...
}
```

This function should:

* Initialize the WebSocket connection using the `URL` and `API Key`.
* Set up event listener threads for connection events (message, metadata, error).
* Start the keep alive timer based on the `Keepalive Interval`.

### Thread: Receive and Process Messages

```perl
class WebSocketClient {
  ...
  function ThreadProcessMessages() {
    // Implement the thread handler to process messages
  }
  ...
}
```

This thread should:

* When a message arrives, check if it's a transcription result or a system message.
* For transcription messages, process or handle the transcription data.
* Handle system messages accordingly (may include error messages or status updates).

### Function: Send

```javascript
class WebSocketClient {
  ...
  function SendBinary([]bytes) {
    // Implements a send function to transport audio to the Deepgram server
  }

  function SendMessage([]byte) {
    // Implements a send function to transport control messages to the Deepgram server 
  }
  ...
}
```

The `SendBinary()` function should:

* Accept audio data as input.
* Send the audio data over the WebSocket connection to Deepgram for processing.

The `SendMessage()` function should:

* Accept JSON data as input.
* Send the JSON over the WebSocket connection to Deepgram for handling control or connection management type functions. A `KeepAlive` or `CloseStream` messages are examples of these types of messages.

If you need more information on the difference, please refer to [RFC-6455](https://datatracker.ietf.org/doc/html/rfc6455).

### (Optional) Thread: KeepAlive

```perl
class WebSocketClient {
  ...
  function ThreadKeepAlive() {
    // Implement the thread handler to process messages
  }
  ...
}
```

This thread is optional providing that audio data is constantly streaming to through the WebSocket; otherwise, it should:

* Regularly send a keepalive message (such as a ping) to Deepgram based on the `Keepalive Interval` to maintain the connection.

Notice this thread is independent of the Receive/Process Message Thread above.

### Function: Close

```perl
class WebSocketClient {
  ...
  function Close() {
    // Implement shutting down the websocket
  }
  ...
}
```

This function should:

* Send a command to close the WebSocket connection.
* Stop the keepalive timer to clean up resources.

## Deepgram API Specifics

Now that you have a basic client, you must handle the Deepgram API specifics. Refer to this documentation for[ more information](/reference/speech-to-text/listen-streaming) .

### Function: Connect

When establishing a connection, you must pass the required parameters defined by the [Deepgram Query Parameters](/reference/speech-to-text/listen-streaming#query-params).

### Thread: Receive and Process Messages

If successfully connected, you should start receiving transcription messages (albeit empty) in the [Response Schema](/reference/speech-to-text/listen-streaming#response-schema) defined below.

<CodeGroup>
  ```json JSON
  {
    "metadata": {
      "transaction_key": "string",
      "request_id": "uuid",
      "sha256": "string",
      "created": "string",
      "duration": 0,
      "channels": 0,
      "models": [
        "string"
      ],
    },
    "type": "Results",
    "channel_index": [
      0,
      0
    ],
    "duration": 0.0,
    "start": 0.0,
    "is_final": boolean,
    "speech_final": boolean,
    "channel": {
      "alternatives": [
        {
          "transcript": "string",
          "confidence": 0,
          "words": [
            {
              "word": "string",
              "start": 0,
              "end": 0,
              "confidence": 0
            }
          ]
        }
      ],
      "search": [
        {
          "query": "string",
          "hits": [
            {
              "confidence": 0,
              "start": 0,
              "end": 0,
              "snippet": "string"
            }
          ]
        }
      ]
    }
  }
  ```
</CodeGroup>

For convenience, you will need to marshal these JSON representations into usable objects/classes to give your users an easier time using your SDK.

### (Optional) Thread: KeepAlive

If you do implement the KeepAlive message, you will need to follow the [guidelines defined here.](/reference/speech-to-text/listen-streaming#stream-keepalive)

### Function: Close

When you are ready to close your WebSocket client, you will need to follow the shutdown [guidelines defined here.](/reference/speech-to-text/listen-streaming#close-stream)

### Special Considerations: Errors

You must be able to handle any protocol-level defined in [RFC-6455](https://datatracker.ietf.org/doc/html/rfc6455) and application-level (i.e., messages from Deepgram) you will need to follow the [guidelines defined here.](/reference/speech-to-text/listen-streaming#error-handling)

## Troubleshooting

Here are some common implementation mistakes.

### My WebSocket Connection Immediately Disconnects

There are usually a few reasons why the Deepgram Platform will terminate the connection:

* No audio data is making it through the WebSocket to the Deepgram Platform. The platform will terminate the connection if no audio data is received in roughly 10 seconds.
* A variation on the above... you have muted the audio source and are no longer sending an audio stream or data.
* The audio encoding is not supported OR the [`encoding`](/docs/encoding) parameter does not match the encoding in the audio stream.
* Invalid connection options via the query parameters are being used. This could be things like misspelling an option or using an incorrect value.

### My WebSocket Connection Disconnects in the Middle of My Conversation

There are usually a few reasons why the Deepgram Platform will terminate the connection (similar to the above):

* You have muted the audio source and are no longer sending an audio stream or data.
* If no audio data is being sent, you must implement the [KeepAlive](/reference/speech-to-text/listen-streaming#stream-keepalive) protocol message.

### My Transcription Messages Are Getting Delayed

There are usually a few reasons why the Deepgram Platform delays sending transcription messages:

* You inadvertently send the [KeepAlive](/reference/speech-to-text/listen-streaming#stream-keepalive) protocol message as a Data or Stream message. This will cause the audio processing to choke or hiccup, thus causing the delay. Please refer to [RFC-6455](https://datatracker.ietf.org/doc/html/rfc6455) to learn more about the difference between data and control messages.
* Network connectivity issues. Please ensure your connection to the Deepgram domain/IP is good. Use `ping` and `traceroute` or `tracert` to map the network path from source to destination.

## Additional Considerations

By adopting object-oriented programming (OOP), the pseudo-code above provides a clear structure for implementing the SDK across different programming languages that support OOP paradigms. This structure facilitates better abstraction, encapsulation, and modularity, making the SDK more adaptable to future changes in the Deepgram API or the underlying WebSocket protocol.

As you implement and refine your SDK, remember that the essence of good software design lies in solving the problem at hand and crafting a solution that's maintainable, extensible, and easy to use.

***





***

title: Determining Your Audio Format for Live Streaming Audio
subtitle: >-
Learn how to determine if your audio is containerized or raw, and what this
means for correctly formatting your requests to Deepgram's API.
slug: docs/determining-your-audio-format-for-live-streaming-audio
-----------------------------------------------------------------

Before you start streaming audio to Deepgram, it’s important that you understand whether your audio is containerized or raw, so you can correctly form your API request.

The difference between containerized and raw audio relates to how much information about the audio is included within the data:

* **Containerized audio stream:** A series of bits is passed along with a header that specifies information about the audio. Containerized audio generally includes enough additional information to allow Deepgram to decode it automatically.
* **Raw audio stream:** The series of bits is passed with no further information. Deepgram needs you to manually provide information about the characteristics of raw audio.

## Streaming Raw Audio

If you’re streaming raw audio to Deepgram, you must provide the [encoding](/docs/encoding/) and [sample rate](/docs/sample-rate/) of your audio stream in your request. Otherwise, Deepgram will be unable to decode the audio and will fail to return a transcript.

An example of a Deepgram API request to stream raw audio:

```
wss://api.deepgram.com/v1/listen?encoding=ENCODING_VALUE&sample_rate=SAMPLE_RATE_VALUE
```

<Info>
  To see a list of raw audio encodings that Deepgram supports, [check out our Encoding documentation](/docs/encoding/).
</Info>

## Streaming Containerized Audio

If you’re streaming containerized audio to Deepgram, you should not set the encoding and sample rate of your audio stream. Instead, Deepgram will read the container’s header and get the correct information for your stream automatically.

An example of a Deepgram API request to stream containerized audio:

```
wss://api.deepgram.com/v1/listen
```

<Info>
  Deepgram supports over 100 different audio formats and encodings. You can see some of the most popular ones at [Supported Audio Format](/docs/supported-audio-formats).
</Info>

## Determining Your Audio Format

If you’re not sure whether your audio is raw or containerized, you can identify audio format in a few different ways.

### Check Documentation

Start by checking any available documentation for your audio source. Often, it will provide details related to audio format. Specifically, check for any mentions of encodings like Opus, Vorbis, PCM, mu-law, A-law, s16, or linear16.

If your audio source is a web API stream, in many cases it will already be containerized. For example, the audio may be raw Opus audio wrapped in an Ogg container or raw PCM audio wrapped in a WAV container.

### Automatically Detect Audio Format

If you’re still not sure whether or not your audio is containerized, you can write an audio stream to disk and try listening to it with a program like VLC. If your audio is containerized, VLC will be able to play it back without any additional configuration.

Alternatively, you can use `ffprobe` (part of the ffmpeg package, which is a cross-platform solution that records, converts, and streams audio and video) to gather information from the audio stream and detect the audio format of a file.

To use `ffprobe`, from a terminal, run:

<CodeGroup>
  ```shell Shell
  ffprobe PATH_TO_FILE
  ```
</CodeGroup>

The last line of the output from this command will include any data `ffprobe` is able to determine about the file’s audio format.

## Using Raw Audio with Encoding & Sample Rate

When using raw audio, make sure to set the [encoding](/docs/encoding/) and the [sample rate](/docs/sample-rate/). Both parameters are required for Deepgram to be able to decode your stream.

***


***

title: Audio Keep Alive
subtitle: Send keep alive messages while streaming audio to keep the connection open.
slug: docs/audio-keep-alive
---------------------------

<div class="flex flex-row gap-2">
  <span class="dg-badge">
    <span><Icon icon="waveform-lines" /> Streaming:Nova</span>
  </span>
</div>

Use the `KeepAlive` message to keep your WebSocket connection open during periods of silence, preventing timeouts and optimizing costs.

## Purpose

Send a `KeepAlive` message every 3-5 seconds to prevent the 10-second timeout that triggers a `NET-0001` error and closes the connection. Ensure the message is sent as a text WebSocket frame—sending it as binary may result in incorrect handling and potential connection issues.

## Example Payloads

To send the `KeepAlive` message, send the following JSON message to the server:

<CodeGroup>
  ```json JSON
  {
    "type": "KeepAlive"
  }
  ```
</CodeGroup>

The server will not send a response back when you send a `KeepAlive` message. If no audio data or `KeepAlive` messages are sent within a 10-second window, the connection will close with a `NET-0001` error.

## Language Specific Implementations

Below are code examples to help you get started using `KeepAlive`.

### Sending a `KeepAlive` message in JSON Format

Construct a JSON message containing the `KeepAlive` type and send it over the WebSocket connection in each respective language.

<CodeGroup>
  ```javascript JavaScript
  const WebSocket = require("ws");

  // Assuming 'headers' is already defined for authorization
  const ws = new WebSocket("wss://api.deepgram.com/v1/listen", { headers });

  // Assuming 'ws' is the WebSocket connection object
  const keepAliveMsg = JSON.stringify({ type: "KeepAlive" });
  ws.send(keepAliveMsg);
  ```

  ```python Python
  import json
  import websocket

  # Assuming 'headers' is already defined for authorization
  ws = websocket.create_connection("wss://api.deepgram.com/v1/listen", header=headers)

  # Assuming 'ws' is the WebSocket connection object
  keep_alive_msg = json.dumps({"type": "KeepAlive"})
  ws.send(keep_alive_msg)
  ```

  ```go Go
  package main

  import (
      "encoding/json"
      "log"
      "net/http"
      "github.com/gorilla/websocket"
  )

  func main() {
      // Define headers for authorization
      headers := http.Header{}

    	// Assuming headers are set here for authorization
      conn, _, err := websocket.DefaultDialer.Dial("wss://api.deepgram.com/v1/listen", headers)
      if err != nil {
          log.Fatal("Error connecting to WebSocket:", err)
      }
      defer conn.Close()

      // Construct KeepAlive message
      keepAliveMsg := map[string]string{"type": "KeepAlive"}
      jsonMsg, err := json.Marshal(keepAliveMsg)
      if err != nil {
          log.Fatal("Error encoding JSON:", err)
      }

      // Send KeepAlive message
      err = conn.WriteMessage(websocket.TextMessage, jsonMsg)
      if err != nil {
          log.Fatal("Error sending KeepAlive message:", err)
      }
  }
  ```

  ```csharp C#
  using System;
  using System.Net.WebSockets;
  using System.Text;
  using System.Threading;
  using System.Threading.Tasks;

  class Program
  {
      static async Task Main(string[] args)
      {
          // Set up the WebSocket URL and headers
          Uri uri = new Uri("wss://api.deepgram.com/v1/listen");

          string apiKey = "DEEPGRAM_API_KEY";

          // Create a new client WebSocket instance
          using (ClientWebSocket ws = new ClientWebSocket())
          {
              // Set the authorization header
              ws.Options.SetRequestHeader("Authorization", "Token " + apiKey);

              // Connect to the WebSocket server
              await ws.ConnectAsync(uri, CancellationToken.None);

              // Construct the KeepAlive message
              string keepAliveMsg = "{\"type\": \"KeepAlive\"}";

              // Convert the KeepAlive message to a byte array
              byte[] keepAliveBytes = Encoding.UTF8.GetBytes(keepAliveMsg);

              // Send the KeepAlive message asynchronously
              await ws.SendAsync(new ArraySegment<byte>(keepAliveBytes), WebSocketMessageType.Text, true, CancellationToken.None);
          }
      }
  }
  ```
</CodeGroup>

### Streaming Examples

Make a streaming request and use `KeepAlive` to keep the connection open.

<CodeGroup>
  ```javascript JavaScript
  const WebSocket = require("ws");

  const authToken = "DEEPGRAM_API_KEY"; // Replace 'DEEPGRAM_API_KEY' with your actual authorization token
  const headers = {
    Authorization: `Token ${authToken}`,
  };

  // Initialize WebSocket connection
  const ws = new WebSocket("wss://api.deepgram.com/v1/listen", { headers });

  // Handle WebSocket connection open event
  ws.on("open", function open() {
    console.log("WebSocket connection established.");

    // Send audio data (replace this with your audio streaming logic)
    // Example: Read audio from a microphone and send it over the WebSocket
    // For demonstration purposes, we're just sending a KeepAlive message

    setInterval(() => {
      const keepAliveMsg = JSON.stringify({ type: "KeepAlive" });
      ws.send(keepAliveMsg);
      console.log("Sent KeepAlive message");
    }, 3000); // Sending KeepAlive messages every 3 seconds
  });

  // Handle WebSocket message event
  ws.on("message", function incoming(data) {
    console.log("Received:", data);
    // Handle received data (transcription results, errors, etc.)
  });

  // Handle WebSocket close event
  ws.on("close", function close() {
    console.log("WebSocket connection closed.");
  });

  // Handle WebSocket error event
  ws.on("error", function error(err) {
    console.error("WebSocket error:", err.message);
  });

  // Gracefully close the WebSocket connection when done
  function closeWebSocket() {
    const closeMsg = JSON.stringify({ type: "CloseStream" });
    ws.send(closeMsg);
  }

  // Call closeWebSocket function when you're finished streaming audio
  // For example, when user stops recording or when the application exits
  // closeWebSocket();
  ```

  ```python Python
  import websocket
  import json
  import time
  import threading

  auth_token = "DEEPGRAM_API_KEY"  # Replace 'DEEPGRAM_API_KEY' with your actual authorization token
  headers = {
      "Authorization": f"Token {auth_token}"
  }

  # WebSocket URL
  ws_url = "wss://api.deepgram.com/v1/listen"

  # Define the WebSocket on_open function
  def on_open(ws):
      print("WebSocket connection established.")
      # Send KeepAlive messages every 3 seconds
      def keep_alive():
          while True:
              keep_alive_msg = json.dumps({"type": "KeepAlive"})
              ws.send(keep_alive_msg)
              print("Sent KeepAlive message")
              time.sleep(3)
      # Start a thread for sending KeepAlive messages
      keep_alive_thread = threading.Thread(target=keep_alive)
      keep_alive_thread.daemon = True
      keep_alive_thread.start()

  # Define the WebSocket on_message function
  def on_message(ws, message):
      print("Received:", message)
      # Handle received data (transcription results, errors, etc.)

  # Define the WebSocket on_close function
  def on_close(ws):
      print("WebSocket connection closed.")

  # Define the WebSocket on_error function
  def on_error(ws, error):
      print("WebSocket error:", error)

  # Create WebSocket connection
  ws = websocket.WebSocketApp(ws_url,
                              on_open=on_open,
                              on_message=on_message,
                              on_close=on_close,
                              on_error=on_error,
                              header=headers)

  # Run the WebSocket
  ws.run_forever()
  ```
</CodeGroup>

## Using Deepgram SDKs

Deepgram's SDKs make it easier to build with Deepgram in your preferred language.
For more information on using Deepgram SDKs, refer to the SDKs documentation in the GitHub Repository.

* [JS SDK](https://github.com/deepgram/deepgram-js-sdk)
* [Python SDK](https://github.com/deepgram/deepgram-python-sdk)
* [Go SDK](https://github.com/deepgram/deepgram-go-sdk)
* [.NET SDK](https://github.com/deepgram/deepgram-dotnet-sdk)

<CodeGroup>
  ```javascript JavaScript
  const { DeepgramClient } = require("@deepgram/sdk");

  const live = async () => {
    const deepgram = new DeepgramClient({ apiKey: "DEEPGRAM_API_KEY" });
    let connection;
    let keepAlive;

    const setupDeepgram = async () => {
      connection = await deepgram.listen.v1.connect({
        model: "nova-3",
        utterance_end_ms: "1500",
        interim_results: "true",
      });

      if (keepAlive) clearInterval(keepAlive);
      keepAlive = setInterval(() => {
        console.log("KeepAlive sent.");
        connection.sendKeepAlive({ type: "KeepAlive" });
      }, 3000); // Sending KeepAlive messages every 3 seconds

      connection.on("open", () => {
        console.log("Connection opened.");
      });

      connection.on("close", () => {
        console.log("Connection closed.");
        clearInterval(keepAlive);
      });

      connection.on("message", (data) => {
        if (data.type === "Metadata") {
          console.log(data);
        } else if (data.type === "Results") {
          console.log(data.channel);
        } else if (data.type === "UtteranceEnd") {
          console.log(data);
        } else if (data.type === "SpeechStarted") {
          console.log(data);
        }
      });

      connection.on("error", (err) => {
        console.error(err);
      });

      connection.connect();
      await connection.waitForOpen();
    };

    setupDeepgram();
  };

  live();
  ```

  ```python Python
  # For more Python SDK migration guides, visit:
  # https://github.com/deepgram/deepgram-python-sdk/tree/main/docs

  import os
  from deepgram import DeepgramClient
  from deepgram.core.events import EventType

  API_KEY = os.getenv("DEEPGRAM_API_KEY")

  def main():
      try:
          deepgram = DeepgramClient(
              api_key=API_KEY,
              config={"keepalive": "true"} # Comment this out to see the effect of not using keepalive
          )

          with deepgram.listen.websocket.v('1').stream(
              model="nova-3",
              language="en-US",
              smart_format=True,
          ) as dg_connection:

              def on_message(result):
                  if hasattr(result, 'channel') and result.channel.alternatives:
                      sentence = result.channel.alternatives[0].transcript
                      if len(sentence) == 0:
                          return
                      print(f"speaker: {sentence}")

              def on_metadata(result):
                  print(f"\n\n{result}\n\n")

              def on_error(error):
                  print(f"\n\n{error}\n\n")

              dg_connection.on(EventType.MESSAGE, on_message)
              dg_connection.on(EventType.METADATA, on_metadata)
              dg_connection.on(EventType.ERROR, on_error)

              dg_connection.start_listening()

      except Exception as e:
          print(f"Could not open socket: {e}")

  if __name__ == "__main__":
      main()
  ```

  ```go Go
  package main

  import (
  	"bufio"
  	"context"
  	"fmt"
  	"os"

  	interfaces "github.com/deepgram/deepgram-go-sdk/pkg/client/interfaces"
  	client "github.com/deepgram/deepgram-go-sdk/pkg/client/live"
  )

  func main() {
  	// init library
  	client.InitWithDefault()

  	// Go context
  	ctx := context.Background()

  	// set the Transcription options
  	tOptions := interfaces.LiveTranscriptionOptions{
  		Model="nova-3",
      Language:  "en-US",
  		Punctuate: true,
  	}

  	// create a Deepgram client
  	cOptions := interfaces.ClientOptions{
  		EnableKeepAlive: true, // Comment this out to see the effect of not using keepalive
  	}

  	// use the default callback handler which just dumps all messages to the screen
  	dgClient, err := client.New(ctx, "", cOptions, tOptions, nil)
  	if err != nil {
  		fmt.Println("ERROR creating LiveClient connection:", err)
  		return
  	}

  	// connect the websocket to Deepgram
  	wsconn := dgClient.Connect()
  	if wsconn == nil {
  		fmt.Println("Client.Connect failed")
  		os.Exit(1)
  	}

  	// wait for user input to exit
  	fmt.Printf("This demonstrates using KeepAlives. Press ENTER to exit...\n")
  	input := bufio.NewScanner(os.Stdin)
  	input.Scan()

  	// close client
  	dgClient.Stop()

  	fmt.Printf("Program exiting...\n")
  }
  ```
</CodeGroup>

## Word Timings

Word timings in streaming transcription results are based on the audio stream itself, not the lifetime of the WebSocket connection. If you send KeepAlive messages without any audio payloads for a period of time, then resume sending audio, the timestamps will continue from where the audio left off—not from when the KeepAlive messages were sent.

Here is an example timeline demonstrating the behavior.

| Event                                                            | Wall Time  | Word Timing Range on Results Response |
| ---------------------------------------------------------------- | ---------- | ------------------------------------- |
| Websocket opened, begin sending audio payloads                   | 0 seconds  | 0 seconds                             |
| Results received                                                 | 5 seconds  | 0-5 seconds                           |
| Results received                                                 | 10 seconds | 5-10 seconds                          |
| Pause sending audio payloads, while sending `KeepAlive` messages | 10 seconds | *n/a*                                 |
| Resume sending audio payloads                                    | 30 seconds | *n/a*                                 |
| Results received                                                 | 35 seconds | 10-15 seconds                         |

***





***

title: Finalize
subtitle: Send a Finalize message to flush the WebSocket stream.
slug: docs/finalize
-------------------

<div class="flex flex-row gap-2">
  <span class="dg-badge">
    <span><Icon icon="waveform-lines" /> Streaming:Nova</span>
  </span>
</div>

Use the `Finalize` message to flush the WebSocket stream. This forces the server to immediately process any unprocessed audio data and return the final transcription results.

## Purpose

In real-time audio processing, there are scenarios where you may need to force the server to process (*or flush*) all unprocessed audio data immediately. Deepgram supports a `Finalize` message to handle such situations, ensuring that interim results are treated as final.

## Example Payloads

To send the `Finalize` message, you need to send the following JSON message to the server:

<CodeGroup>
  ```json JSON
  {
    "type": "Finalize"
  }
  ```
</CodeGroup>

You can optionally specify a `channel` field to finalize a specific channel. If the `channel` field is omitted, all channels in the audio will be finalized. Note that channel indexing starts at 0, so to finalize only the *first* channel you need to send:

<CodeGroup>
  ```json JSON
  {
    "type": "Finalize",
     "channel": 0
  }
  ```
</CodeGroup>

Upon receiving the Finalize message, the server will process all remaining audio data and return the final results. You may receive a response with the `from_finalize` attribute set to `true`, indicating that the finalization process is complete. This response typically occurs when there is a noticeable amount of audio buffered in the server.

If you specified a `channel` to be finalized, use the response's `channel_index` field to check which channel was finalized.

<CodeGroup>
  ```json JSON
  {
    "from_finalize": true
  }
  ```
</CodeGroup>

<Info>
  In most cases, you will receive this response, but it is not guaranteed if there is no significant amount of audio data to process.
</Info>

## Language-Specific Implementations

Below are code examples to help you get started using `Finalize`.

### Sending a `Finalize` message in JSON Format

These snippets demonstrate how to construct a JSON message containing the "Finalize" type and send it over the WebSocket connection in each respective language.

<CodeGroup>
  ```javascript JavaScript
  const WebSocket = require("ws");

  // Assuming 'headers' is already defined for authorization
  const ws = new WebSocket("wss://api.deepgram.com/v1/listen", { headers });

  ws.on('open', function open() {
    // Construct Finalize message
    const finalizeMsg = JSON.stringify({ type: "Finalize" });

    // Send Finalize message
    ws.send(finalizeMsg);
  });
  ```

  ```python Python
  import json
  import websocket

  # Assuming 'headers' is already defined for authorization
  ws = websocket.create_connection("wss://api.deepgram.com/v1/listen", header=headers)

  # Construct Finalize message
  finalize_msg = json.dumps({"type": "Finalize"})

  # Send Finalize message
  ws.send(finalize_msg)
  ```

  ```go Go
  package main

  import (
      "encoding/json"
      "log"
      "net/http"
      "github.com/gorilla/websocket"
  )

  func main() {
      // Define headers for authorization
      headers := http.Header{}

      // Assuming headers are set here for authorization
      conn, _, err := websocket.DefaultDialer.Dial("wss://api.deepgram.com/v1/listen", headers)
      if err != nil {
          log.Fatal("Error connecting to WebSocket:", err)
      }
      defer conn.Close()

      // Construct Finalize message
      finalizeMsg := map[string]string{"type": "Finalize"}
      jsonMsg, err := json.Marshal(finalizeMsg)
      if err != nil {
          log.Fatal("Error encoding JSON:", err)
      }

      // Send Finalize message
      err = conn.WriteMessage(websocket.TextMessage, jsonMsg)
      if err != nil {
          log.Fatal("Error sending Finalize message:", err)
      }
  }
  ```

  ```csharp C#
  using System;
  using System.Net.WebSockets;
  using System.Text;
  using System.Threading;
  using System.Threading.Tasks;

  class Program
  {
      static async Task Main(string[] args)
      {
          // Set up the WebSocket URL and headers
          Uri uri = new Uri("wss://api.deepgram.com/v1/listen");

          string apiKey = "DEEPGRAM_API_KEY";

          // Create a new client WebSocket instance
          using (ClientWebSocket ws = new ClientWebSocket())
          {
              // Set the authorization header
              ws.Options.SetRequestHeader("Authorization", "Token " + apiKey);

              // Connect to the WebSocket server
              await ws.ConnectAsync(uri, CancellationToken.None);

              // Construct the Finalize message
              string finalizeMsg = "{\"type\": \"Finalize\"}";

              // Convert the Finalize message to a byte array
              byte[] finalizeBytes = Encoding.UTF8.GetBytes(finalizeMsg);

              // Send the Finalize message asynchronously
              await ws.SendAsync(new ArraySegment<byte>(finalizeBytes), WebSocketMessageType.Text, true, CancellationToken.None);
          }
      }
  }
  ```
</CodeGroup>

### Streaming Examples

Here are more complete examples that make a streaming request and use Finalize. Try running these examples to see how Finalize can be sent to Deepgram, forcing the API to process all unprocessed audio data and immediately return the results.

<CodeGroup>
  ```javascript JavaScript
  const WebSocket = require("ws");
  const axios = require("axios");
  const { PassThrough } = require("stream");

  const apiKey = "YOUR_DEEPGRAM_API_KEY";
  const headers = {
    Authorization: `Token ${apiKey}`,
  };

  // Initialize WebSocket connection
  const ws = new WebSocket("wss://api.deepgram.com/v1/listen", { headers });

  ws.on("open", async function open() {
    console.log("WebSocket connection established.");

    try {
      // Fetch the audio stream from the remote URL
      const response = await axios({
        method: "get",
        url: "http://stream.live.vc.bbcmedia.co.uk/bbc_world_service",
        responseType: "stream",
      });

      const passThrough = new PassThrough();
      response.data.pipe(passThrough);

      passThrough.on("data", (chunk) => {
        ws.send(chunk);
      });

      passThrough.on("end", () => {
        console.log("Audio stream ended.");
        finalizeWebSocket();
      });

      passThrough.on("error", (err) => {
        console.error("Stream error:", err.message);
      });

      // Send Finalize message after 10 seconds
      setTimeout(() => {
        finalizeWebSocket();
      }, 10000);
    } catch (error) {
      console.error("Error fetching audio stream:", error.message);
    }
  });

  // Handle WebSocket message event
  ws.on("message", function incoming(data) {
    let response = JSON.parse(data);
    if (response.type === "Results") {
      console.log("Transcript: ", response.channel.alternatives[0].transcript);
    }
  });

  // Handle WebSocket close event
  ws.on("close", function close() {
    console.log("WebSocket connection closed.");
  });

  // Handle WebSocket error event
  ws.on("error", function error(err) {
    console.error("WebSocket error:", err.message);
  });

  // Send Finalize message to WebSocket
  function finalizeWebSocket() {
    const finalizeMsg = JSON.stringify({ type: "Finalize" });
    ws.send(finalizeMsg);
    console.log("Finalize message sent.");
  }

  // Gracefully close the WebSocket connection when done
  function closeWebSocket() {
    const closeMsg = JSON.stringify({ type: "CloseStream" });
    ws.send(closeMsg);
    ws.close();
  }

  // Close WebSocket when process is terminated
  process.on("SIGINT", () => {
    closeWebSocket();
    process.exit();
  });
  ```

  ```python Python
  from websocket import WebSocketApp
  import websocket
  import json
  import threading
  import requests
  import time

  auth_token = "YOUR_DEEPGRAM_API_KEY"  # Replace with your actual authorization token

  headers = {
      "Authorization": f"Token {auth_token}"
  }

  # WebSocket URL
  ws_url = "wss://api.deepgram.com/v1/listen"

  # Audio stream URL
  audio_url = "http://stream.live.vc.bbcmedia.co.uk/bbc_world_service"

  # Define the WebSocket functions on_open, on_message, on_close, and on_error

  def on_open(ws):
      print("WebSocket connection established.")

      # Start audio streaming thread
      audio_thread = threading.Thread(target=stream_audio, args=(ws,))
      audio_thread.daemon = True
      audio_thread.start()

      # Finalize test thread
      finalize_thread = threading.Thread(target=finalize_test, args=(ws,))
      finalize_thread.daemon = True
      finalize_thread.start()

  def on_message(ws, message):
      try:
          response = json.loads(message)
          if response.get("type") == "Results":
              transcript = response["channel"]["alternatives"][0].get("transcript", "")
              if transcript:
                  print("Transcript:", transcript)

              # Check if this is the final result from finalize
              # Note: in most cases, you will receive this response, but it is not guaranteed if there is no significant amount of audio data left to process.
              if response.get("from_finalize", False):
                  print("Finalization complete.")
      except json.JSONDecodeError as e:
          print(f"Error decoding JSON message: {e}")
      except KeyError as e:
          print(f"Key error: {e}")

  def on_close(ws, close_status_code, close_msg):
      print(f"WebSocket connection closed with code: {close_status_code}, message: {close_msg}")

  def on_error(ws, error):
      print("WebSocket error:", error)

  # Define the function to stream audio to the WebSocket

  def stream_audio(ws):
      response = requests.get(audio_url, stream=True)
      if response.status_code == 200:
          print("Audio stream opened.")
          for chunk in response.iter_content(chunk_size=4096):
              ws.send(chunk, opcode=websocket.ABNF.OPCODE_BINARY)
      else:
          print("Failed to open audio stream:", response.status_code)

  # Define the function to send the Finalize message

  def finalize_test(ws):
      # Wait for 10 seconds before sending the Finalize message to simulate the end of audio streaming
      time.sleep(10)
      finalize_message = json.dumps({"type": "Finalize"})
      ws.send(finalize_message)
      print("Finalize message sent.")

  # Create WebSocket connection

  ws = WebSocketApp(ws_url, on_open=on_open, on_message=on_message, on_close=on_close, on_error=on_error, header=headers)

  # Run the WebSocket

  ws.run_forever()
  ```
</CodeGroup>

***





***

title: Close Stream
subtitle: Send a CloseStream message to close the WebSocket stream.
slug: docs/close-stream
-----------------------

<div class="flex flex-row gap-2">
  <span class="dg-badge">
    <span><Icon icon="waveform-lines" /> Streaming:Nova</span>
  </span>
</div>

Use the `CloseStream` message to close the WebSocket stream. This forces the server to immediately process any unprocessed audio data and return the final transcription results.

## Purpose

In real-time audio processing, there are scenarios where you may need to force the server to close. Deepgram supports a `CloseStream` message to handle such situations. This message will send a shutdown command to the server instructing it to finish processing any cached data, send the response to the client, send a summary metadata object, and then terminate the WebSocket connection.

## Example Payloads

To send the `CloseStream` message, you need to send the following JSON message to the server:

<CodeGroup>
  ```json JSON
  {
    "type": "CloseStream"
  }
  ```
</CodeGroup>

Upon receiving the `CloseStream` message, the server will process all remaining audio data and return the following:

<CodeGroup>
  ```json JSON
  {
      "type": "Metadata",
      "transaction_key": "deprecated",
      "request_id": "8c8ebea9-dbec-45fa-a035-e4632cb05b5f",
      "sha256": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "created": "2024-08-29T22:37:55.202Z",
      "duration": 0.0,
      "channels": 0
  }
  ```
</CodeGroup>

## Language-Specific Implementations

Below are code examples to help you get started using `CloseStream`.

<CodeGroup>
  ```javascript JavaScript
  const WebSocket = require("ws");

  // Assuming 'headers' is already defined for authorization
  const ws = new WebSocket("wss://api.deepgram.com/v1/listen", { headers });

  ws.on('open', function open() {
    // Construct CloseStream message
    const closeStreamMsg = JSON.stringify({ type: "CloseStream" });

    // Send CloseStream message
    ws.send(closeStreamMsg);
  });
  ```

  ```python Python
  import json
  import websocket

  # Assuming 'headers' is already defined for authorization
  ws = websocket.create_connection("wss://api.deepgram.com/v1/listen", header=headers)

  # Construct CloseStream message
  closestream_msg = json.dumps({"type": "CloseStream"})

  # Send CloseStream message
  ws.send(closestream_msg)
  ```

  ```go Go
  package main

  import (
      "encoding/json"
      "log"
      "net/http"
      "github.com/gorilla/websocket"
  )

  func main() {
      // Define headers for authorization
      headers := http.Header{}
      headers.Add("Authorization", "Bearer YOUR_API_KEY") // Replace with your actual API key

      // Connect to the WebSocket server
      conn, _, err := websocket.DefaultDialer.Dial("wss://api.deepgram.com/v1/listen", headers)
      if err != nil {
          log.Fatal("Error connecting to WebSocket:", err)
      }
      defer conn.Close()

      // Construct CloseStream message
      closeStreamMsg := map[string]string{"type": "CloseStream"}
      jsonMsg, err := json.Marshal(closeStreamMsg)
      if err != nil {
          log.Fatal("Error encoding JSON:", err)
      }

      // Send CloseStream message
      err = conn.WriteMessage(websocket.TextMessage, jsonMsg)
      if err != nil {
          log.Fatal("Error sending CloseStream message:", err)
      }
  }
  ```

  ```csharp C#
  using System;
  using System.Net.WebSockets;
  using System.Text;
  using System.Threading;
  using System.Threading.Tasks;

  class Program
  {
      static async Task Main(string[] args)
      {
          // Set up the WebSocket URL and headers
          Uri uri = new Uri("wss://api.deepgram.com/v1/listen");
          string apiKey = "YOUR_API_KEY"; // Replace with your actual API key

          // Create a new client WebSocket instance
          using (ClientWebSocket ws = new ClientWebSocket())
          {
              // Set the authorization header
              ws.Options.SetRequestHeader("Authorization", "Token " + apiKey);

              try
              {
                  // Connect to the WebSocket server
                  await ws.ConnectAsync(uri, CancellationToken.None);

                  // Construct the CloseStream message
                  string closeStreamMsg = "{\"type\": \"CloseStream\"}";

                  // Convert the CloseStream message to a byte array
                  byte[] finalizeBytes = Encoding.UTF8.GetBytes(closeStreamMsg);

                  // Send the CloseStream message asynchronously
                  await ws.SendAsync(new ArraySegment<byte>(finalizeBytes), WebSocketMessageType.Text, true, CancellationToken.None);
              }
              catch (WebSocketException ex)
              {
                  Console.WriteLine("WebSocket error: " + ex.Message);
              }
              catch (Exception ex)
              {
                  Console.WriteLine("General error: " + ex.Message);
              }
          }
      }
  }
  ```
</CodeGroup>

***






***

title: Errors
subtitle: Errors you might encounter when making requests to the Deepgram API
slug: docs/errors
-----------------

A record of errors and reasons you will receive them when using the Deepgram API.

## General API errors

Errors that could be returned on any endpoint.

### `400` Invalid JSON submitted

When making a `POST` request with JSON data, you must include all required fields. If required filed are missing, or the submitted JSON is invalid, a `400 Bad Request` will be returned. The response will be similar to the below, depending on the endpoint and how the JSON is malformed.

```json
{
  "category": "INVALID_JSON",
  "message": "Invalid JSON submitted.",
  "details": "Json deserialize error: missing field `xxx` at line 7 column 1",
  "request_id": "uuid"
}
---
{
  "err_code": "Bad Request",
  "err_msg": "Content-type was application/json, but we could not process the JSON payload.",
  "request_id": "uuid"
}
---
{
  "category": "INVALID_JSON",
  "message": "Invalid JSON submitted.",
  "details": "Json deserialize error: expected `:` at line 3 column 13",
  "request_id": "uuid"
}
```

### `400` Unknown request body format

If you receive the following error:

```json
{
  "err_code": "Bad Request",
  "err_msg": "Bad Request: failed to process audio: corrupt or unsupported data",
  "request_id": "uuid"
}
```

Often, this is caused by sending Deepgram a URL to transcribe, but failing to set a `Content-Type: application/json` header. When sending Deepgram a JSON payload containing a URL, the `Content-Type: application/json` must be set in the request.

If you are sending an audio file and not a URL, you may be sending corrupted audio. You can use tools such as `ffprobe` or Audacity to confirm that your audio file is valid.

### `401` Incorrect API key

Providing an invalid API key will return `401 Unauthorized` with the following error.

```json
{
  "err_code": "INVALID_AUTH",
  "err_msg": "Invalid credentials.",
  "request_id": "uuid"
}
```

### `401` Insufficient permissions

Making a request that you do not have sufficient permissions for will return `401 Unauthorized` with this error.

```json
{
 "err_code":"INSUFFICIENT_PERMISSIONS",
 "err_msg":"User does not have sufficient permissions.",
 "request_id":"uuid"
}
```

### `403` Insufficient permissions

Making a request for a model that you do not have access to will return `403 Forbidden` with this error.

```json
{
 "err_code":"INSUFFICIENT_PERMISSIONS",
 "err_msg":"Project does not have access to the requested model.",
 "request_id":"uuid"
}
```

### `404` UUID parsing failed

Providing an invalid Project ID will fail parsing and return `404 Not Found` and this response.

```text
UUID parsing failed: invalid character: expected an optional prefix of `urn:uuid:` followed by [0-9a-zA-Z], found `p` at 1
```

### `404` Project not found

When a project isn't found it will result in `404 Not Found`. It may be because;

* the Project ID is incorrect
* the Project ID is for a project that has been deleted
* the Project ID is not associated with the API key used to make the request

```json
{
  "err_code": "PROJECT_NOT_FOUND",
  "err_msg": "Project not found."
}
```

## Speech to Text errors

### `402` Insufficient credits

When attempting to transcribe a file, you may not have sufficient funds to complete the request. This will result in a `402 Payment Required` error with this error.

```json
{
  "err_code": "ASR_PAYMENT_REQUIRED",
  "err_msg": "Project does not have enough credits for an ASR request and does not have an overage agreement.",
  "request_id": "uuid"
}
```

### `422` Unprocessable Entity

When attempting to transcribe audio, Deepgram was unable to process the request because the audio data was incomplete or interrupted. This typically occurs when the connection is closed before the full audio payload is received, or when upload speed is too slow and the upload times out.

You can check client-side logs for evidence of interrupted uploads or timeouts. If the issue persists, contact support with the request ID and details about how the audio was uploaded. Including the original audio file, if possible, can help with troubleshooting.

```json
{
  "err_code": "ASR_UNPROCESSABLE_ENTITY",
  "err_msg": "Unable to read the entire client request.",
  "request_id": "uuid"
}
```

### `429` Rate limit exceeded

When requests are made in excess of Deepgram's [rate limits](https://developers.deepgram.com/docs/getting-started-with-pre-recorded-audio#rate-limits), a `429 Too Many Requests` is returned with the following error. An [exponential-backoff retry strategy](https://deepgram.com/learn/api-back-off-strategies) is recommended to accommodate rate-limiting when submitting a large volume of concurrent requests.

```json json
{
  "err_code": "TOO_MANY_REQUESTS",
  "err_msg": "Too many requests. Please try again later",
  "request_id": "uuid"
}
```

## Text to Speech Errors

### `400` Unknown Model. Query parameters specify a model that does not exist.

The model requested is not one of Deepgram's [voice models](/docs/tts-models).

```json
{
  "err_code": "Bad Request",
  "err_msg": "Bad Request: No such model/language/tier combination found.",
  "request_id": "[unique_request_id]"
}
```

### `400` Failure to Parse Query Parameters

The query parameters were invalid. The `message` can be anything describing a failure to parse an invalid query string.

```json
{
  "err_code": "INVALID_QUERY_PARAMETER",
  "err_msg": "Failed to deserialize query parameters: [message]",
  "request_id": "[unique_request_id]"
}
```

### `400` Input Text Contained No Characters

The text payload contained no characters, resulting in Deepgram being unable to synthesize text into audio.

```json
{
  "err_code": "Bad Request",
  "err_msg": "Input text contains no characters.",
  "request_id": "[unique_request_id]"
}
```

### `400` Unsupported Output Audio Format Requested in Query Parameters

The request provides a query string containing any combination of query parameters that describes an unsupported output audio format.

```json
{
  "err_code": "INVALID_QUERY_PARAMETER",
  "err_msg": "Unsupported audio format: [message]",
  "request_id": "[unique_request_id]"
}
```

One or more of the following query parameters is unsupported:

* `encoding=[encoding]`
* `container=[container]`
* `sample_rate=[sample_rate]`
* `bit_rate=[bit_rate]`

`message` may be any one of:

* "`container` is not applicable when `encoding=[encoding]`"
* "`container=[container]` is invalid when `encoding=[encoding]`"
* "`sample_rate` is not applicable when `encoding=[encoding]`"
* "`sample_rate` must be \[list of valid sample rates] when `encoding=[encoding]`"
* "`bit_rate` is not applicable when `encoding=[encoding]`"
* "`bit_rate` must be \[list of valid bit rates] when `encoding=[encoding]`"

### `400` Failure to Parse Request Body as JSON

The request body did not deserialize as JSON successfully. The request body must specify exactly one of `text` or `url` in the body.

```json
{
  "err_code": "PAYLOAD_ERROR",
  "err_msg": "Failed to deserialize JSON payload. Please specify exactly one of `text` or `url` in the JSON body.",
  "request_id": "[unique_request_id]"
}
```

### `400` Failure to Parse Remote Text URL Provide in JSON

There was a failure to parse a remote text URL provided within the JSON body.

```json
{
  "err_code": "PAYLOAD_ERROR",
  "err_msg": "Failed to parse URL in JSON body.",
  "request_id": "[unique_request_id]"
}
```

### `400` Failure to Fetch Remote Text from URL

There was a failure to retrieve remote text content from the specified URL.

```json
{
  "err_code": "REMOTE_CONTENT_ERROR",
  "err_msg": "[message]",
  "request_id": "[unique_request_id]"
}
```

`message` may be any of:

* "Failed to deserialize remote text data. Please provide `application/json` with a `text` field or `text/plain`."
* "URL for media download must be publicly routable."
* "Could not determine if URL for media download is publicly routable."
* "Could not parse URL as a URI."
* "The remote server hosting the media failed to include a `location` header in its redirect response."
* "Could not parse remote media server's redirect location as a valid UTF-8 string."
* "Could not parse remote media server's redirect location as a URL."
* "The remote server hosting the media returned a client error: \[HTTP status]."
* "The remote server hosting the media failed to return valid data."
* "The remote server hosting the media returned too many redirects."

### `400` Invalid Callback

The provided callback url was invalid.

```json
{
  "err_code": "INVALID_QUERY_PARAMETER",
  "err_msg": "Invalid callback url.",
  "request_id": "[unique_request_id]"
}
```

### `413` Request Body Exceeded 2MB

The request body exceeded the 2MB limit, indicating that the payload size is too large to be processed.

```json
{
  "err_code": "PAYLOAD_TOO_LARGE",
  "err_msg": "Payload size exceeds limit of 2 MB.",
  "request_id": "[unique_request_id]"
}
```

### `413` Input Text Exceeded Character Limit

The text payload contained more than the maximum number of characters allowed.

```json
{
  "err_code": "Payload Too Large",
  "err_msg": "Input text exceeds maximum character limit of [max_characters].",
  "request_id": "[unique_request_id]"
}
```

### `400` Failure to Decode Request Body as UTF-8

The payload cannot be decoded because it is not encoded as UTF-8.

```json
{
  "err_code": "PAYLOAD_ERROR",
  "err_msg": "Failed to decode payload as UTF-8.",
  "request_id": "[unique_request_id]"
}
```

### `415` Unsupported Content Type in Request

The `Content-Type` header in the request is not supported, requiring it to be either `text/plain` or `application/json`.

```json
{
  "err_code": "UNSUPPORTED_MEDIA_TYPE",
  "err_msg": "`Content-Type` header is not supported. `Content-Type` must be either `text/plain` or `application/json`.",
  "request_id": "[unique_request_id]"
}
```

### `422` Unprocessable Content

The model failed to process the request.

```json
{
  "err_code": "UNPROCESSABLE_ENTITY",
  "err_msg": "Failed to handle request.",
  "request_id": "[unique_request_id]"
}
```

### `429` Rate Limit Exceeded

When requests are made in excess of Deepgram's rate limits.

```json
{
  "err_code": "Too Many Requests",
  "err_msg": "Please try again later.",
  "request_id": "[unique_request_id]"
}
```

* Learn about strategies for handling 429 errors in our [Help Center.](https://deepgram.gitbook.io/help-center/faq/how-should-i-handle-429-rate-limit-responses)

### `503` Service Unavailable

```json

{
  "error_code":"Service Unavailable",
  "err_msg": "Please try again later",
  "request_id": "[unique_request_id]"
}
```

## Handling HTTP Errors

### Production

Some error codes, such as `400: Bad Request` errors, can be prevented in your production code by careful testing and development. However, others, such as `503: Service Unavailable`, can occur regardless of your implementation.

Below is a list of HTTP error codes that your production code should handle gracefully. Some of these errors may succeed if retried, while others (such as `414: URI Too Long`) need to be handled by modifying the request.

* **408 Request Timeout**: The server timed out waiting for the request.
* **411 Length Required**: The server refuses to accept the request without a defined Content-Length.
* **413 Request Entity Too Large**: The request is larger than the server is willing or able to process.
* **414 URI Too Long**: The server is refusing to service the request because the request-target is longer than the server is willing to interpret.
* **429 Too Many Requests**: The user has sent too many requests in a given amount of time.
* **499 Client Closed Request**: A non-standard status code indicating that the client closed the connection.
* **500 Internal Server Error**: A generic error message indicating that the server has encountered a situation it doesn't know how to handle.
* **502 Bad Gateway**: The server, while acting as a gateway or proxy, received an invalid response from the upstream server it accessed in attempting to fulfill the request.
* **503 Service Unavailable**: The server is not ready to handle the request. Common causes include a server that is down for maintenance or is overloaded.
* **504 Gateway Timeout**: The server, while acting as a gateway or proxy, did not receive a timely response from the upstream server or some other auxiliary server it needed to access in order to complete the request.

### Development

The following errors are more likely to be encountered in a development environment. You may want to add error handling in your production code to gracefully handle these error codes as well.

* **400 Bad Request**: The server could not understand the request due to invalid syntax.
* **401 Insufficient permissions**: The project does not have permissions to access the requested features or model.
* **401 Unauthorized**: The API key is invalid or unauthorized.
* **402 Payment Required**: The project has insufficient funds to complete the request.
* **403 Forbidden**: The server understood the request but refuses to authorize it.
* **404 Not Found**: The specified entity ID could not be found.




***

title: 'STT Troubleshooting WebSocket, NET, and DATA Errors'
subtitle: 'Learn how to debug common real-time, live streaming transcription errors.'
slug: docs/stt-troubleshooting-websocket-data-and-net-errors
------------------------------------------------------------

When working with Deepgram's Speech To Text Streaming API, you may encounter WebSocket errors. This troubleshooting guide helps you quickly identify and resolve the most common issues.

## WebSocket Basics

* WebSocket enables two-way, real-time communication between client and server.
* The connection is established via an HTTP handshake and upgraded to WebSocket.
* If the handshake fails, you'll get an HTTP `4xx` or `5xx` error.
* The connection stays open until closed by either side.

### Establishing a WebSocket Connection

* The client initiates a WebSocket connection with an HTTP handshake, optionally including query parameters or headers (for authentication, etc.).
* Most libraries handle the handshake automatically (e.g., `websockets.connect`).
* If successful, the server responds with HTTP `101` and upgrades the connection.
* If unsuccessful, you'll receive an HTTP `4xx` or `5xx` error and the connection won't be established.

### Closing the WebSocket Connection

* A successfully opened WebSocket connection will stay alive until it is eventually closed by either the client or the server. When this occurs, a [WebSocket Close Frame](https://tools.ietf.org/html/rfc6455#section-5.5.1) will be returned.
* The body of the Close frame will indicate the reason for closing with a [pre-defined status code](https://tools.ietf.org/html/rfc6455#section-7.4.1) followed by a UTF-8-encoded payload that represents the reason for the error.
* To close the WebSocket connection from your client, send a [Close Stream](/docs/close-stream) message. The server will then finish processing any remaining data, send a final response and summary metadata, and terminate the connection.
* After sending a Close message, the endpoint considers the WebSocket connection closed and will close the underlying TCP connection.

<Warning>
  Sending an empty byte (e.g., `b''`) will cause unexpected closures. Avoid sending an empty byte accidentally by adding a conditional to check if the length of your audio packet is 0 before sending.
</Warning>

## Using KeepAlive Messages to Prevent Timeouts

* Send a [KeepAlive](/docs/audio-keep-alive) message periodically to keep the connection open.
* Doing this can prevent timeouts and NET-0001 errors (no audio received for 10 seconds).

## Common WebSocket Errors

### Failure to Connect

If a failure to connect occurs, Deepgram returns custom HTTP headers for debugging:

* `dg-request-id`: Always present, contains the request ID.
* `dg-error`: Present on failed upgrades, contains the error message.

<Info>
  Access to these headers will depend on the WebSocket library you are using. For example, browser-based WebSocket libraries like the JavaScript WebSocket library only allow access to HTTP header information for successful WebSocket connections.
</Info>

### Debugging Connection Failures

If you're unable to connect the Deepgram API provides custom HTTP headers that contain debugging information:

* Regardless of the success or failure of the WebSocket upgrade, all requests include the `dg-request-id` HTTP header, which contains the request ID.
* Requests that do not successfully upgrade to a WebSocket connection also include the `dg-error` HTTP header, which contains a detailed error message concerning why the connection could not be upgraded. This error message is also sent back in the body of the HTTP response.

### Code Samples

These code samples demonstrate how to connect to Deepgram’s API using WebSockets, authenticate with your API key, and handle both successful and failed connection attempts by printing relevant request IDs and error messages for troubleshooting.

<Warning>
  Replace `YOUR_DEEPGRAM_API_KEY` with your [Deepgram API Key](/docs/create-additional-api-keys).
</Warning>

<CodeGroup>
  ```python Python
  import websockets
  import json
  import asyncio

  async def main():
      try:
          async with websockets.connect('wss://api.deepgram.com/v1/listen',
          # Remember to replace the YOUR_DEEPGRAM_API_KEY placeholder with your Deepgram API Key
          extra_headers = { 'Authorization': f'token YOUR_DEEPGRAM_API_KEY' }) as ws:
              # If the request is successful, print the request ID from the HTTP header
              print('🟢 Successfully opened connection')
              print(f'Request ID: {ws.response_headers["dg-request-id"]}')
              await ws.send(json.dumps({
                  'type': 'CloseStream'
              }))
      except websockets.exceptions.InvalidStatusCode as e:
          # If the request fails, print both the error message and the request ID from the HTTP headers
          print(f'🔴 ERROR: Could not connect to Deepgram! {e.headers.get("dg-error")}')
          print(f'🔴 Please contact Deepgram Support with request ID {e.headers.get("dg-request-id")}')

  asyncio.run(main())
  ```

  ```javascript JavaScript
  const WebSocket = require('ws');
  const ws = new WebSocket('wss://api.deepgram.com/v1/listen', {
      headers: {
        // Remember to replace the YOUR_DEEPGRAM_API_KEY placeholder with your Deepgram API Key
        Authorization: 'Token YOUR_DEEPGRAM_API_KEY',
      },
  });
  // For security reasons, browser-based WebSocket libraries only allow access to HTTP header information for successful WebSocket connections
  // If the request is successful, return the HTTP header that contains the request ID
  ws.on('upgrade', function message(data) {
      console.log(data.headers['dg-request-id']);
  });
  ```
</CodeGroup>

### Abrupt WebSocket Closures

If Deepgram encounters an error during real-time streaming, the Deepgram API returns a [WebSocket Close frame](https://www.rfc-editor.org/rfc/rfc6455#section-5.5.1). The body of the Close frame will indicate the reason for closing with a [pre-defined status code](https://tools.ietf.org/html/rfc6455#section-7.4.1) followed by a UTF-8-encoded payload that represents the reason for the error.

Below are the most common WebSocket Close frame status codes and their descriptions.

| Code   | Payload     | Description                                                                                                                                                                                                                   |
| ------ | ----------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `1008` | `DATA-0000` | The payload cannot be decoded as audio. The payload either is not audio data or is a codec unsupported by Deepgram.                                                                                                           |
| `1011` | `NET-0000`  | The service has not transmitted a Text frame to the client within the timeout window. This may indicate an internal issue with Deepgram's systems, or Deepgram may have not received enough audio data to transcribe a frame. |
| `1011` | `NET-0001`  | The service has not received a Binary or Text frame from the client within the timeout window. This may indicate an internal issue with Deepgram's systems, the client's systems, or the network connecting them.             |

#### Troubleshooting `1008` - `DATA-0000`

* Check the data being sent is valid audio.
* Check the audio data is not empty.
* If the audio data is valid, check whether the audio being sent is raw or containerized.
* Write the audio data to a file to make sure it contains the expected audio and can be played back.
* Ensure [Encoding](/docs/encoding) and [Sample Rate](/docs/sample-rate) parameters are set correctly.
* See [Audio Format For Live Streaming](/docs/determining-your-audio-format-for-live-streaming-audio) for more information.

#### Troubleshooting `1011` - `NET-0000`

* This indicates an internal server error.
* Retry your request.
* Check [Deepgram status](https://status.deepgram.com/) to see if there are any ongoing issues.
* If Deepgram is operational, [contact Support](/support) for assistance.

#### Troubleshooting `1011` - `NET-0001`

* Ensure audio is sent within 10 seconds of opening the connection.
* You can send silent audio to keep the connection alive.
* Using `KeepAlive` messages alone will not prevent closure; you must send at least one audio message.
* Be sure to send a [Close Stream](/docs/close-stream) message when done.
* Test your network with cURL and Deepgram-hosted audio. See [Generating Transcripts from the Terminal](/docs/generating-and-saving-transcripts-from-the-terminal) for more information.
* Use a tool like [Wireshark](https://www.wireshark.org/) to confirm audio is leaving your network.

***




***

title: Live Streaming Starter Kit
subtitle: >-
Deepgram's Live Streaming Starter Kit will take you step by step through the
process of getting up and running with Deepgram's live streaming API.
slug: docs/getting-started-with-the-streaming-test-suite
--------------------------------------------------------

If you're looking to get started with Deepgram's audio streaming capabilities, this is the perfect place to begin. The starter kit provides sample code that allows you to easily stream basic audio to Deepgram, ensuring that you have the necessary foundation to build for your unique use case.

![image of terminal showing the streaming test suite transcribing the Preamble](file:89eb7aa6-90cb-4e7b-9d71-8fde9a71190c)

Once you've tested out the basics of streaming audio to Deepgram, you'll move on to using an included mock server for testing. This allows you to focus on getting your audio and client code right. Once you're confident that your audio stream is configured correctly and you're streaming the audio you expect, you can easily swap to sending that audio to Deepgram's service.

Before diving into writing code from scratch, we highly recommend running through the steps in our starter kit at least once to ensure that you can stream sample audio to Deepgram successfully. This will help you avoid many potential issues and streamline the integration process for sending your own audio to our system.

The starter kit includes many ways to help diagnose problems including more details on errors as well as steps to fix the errors you encounter.

# Set Up

## Prerequisites

You must have:

* Python >= 3.6+
* [portaudio](http://portaudio.com/), if you plan to stream audio from your microphone
* A valid Deepgram API key (you can create one in our [Console](https://console.deepgram.com/signup?jump=keys))

## Installation

1. Clone the [Live Streaming Starter Kit](https://github.com/deepgram/streaming-test-suite/) repository
2. Install [portaudio](http://portaudio.com/)
3. `pip install -r requirements.txt`

<Info>
  <h2> Installing PortAudio </h2>
  If you use Homebrew or Conda, we recommend installing PortAudio with `brew install portaudio` or `conda install portaudio`.

  Otherwise, you can download a zip file from [portaudio.com](http://portaudio.com/), unzip it, and then consult [PortAudio's docs](http://www.portaudio.com/docs/v19-doxydocs/pages.html) as a reference for how to build the package on your operating system. For Linux and MacOS, the build command within the top-level `portaudio/` directory is `./configure && make`.

  PortAudio is known to have compatibility issues on Windows. However, this dependency is only required if you plan to stream audio from your microphone. If you run into issues installing PortAudio, you can still complete the other tasks outlined in this guide.
</Info>

# Streaming a Local Source

The first step in getting started with Deepgram's audio streaming capabilities is to learn how to stream a local audio source to Deepgram. This task allows you to learn the basic concepts of how Deepgram's API works without worrying about complexities that arise with other audio sources. Additionally, it ensures that you can receive results from Deepgram in your development environment.

The starter kit provides sample code that facilitates this process. Before building your own integration, we recommend running this code at least once to make sure that you can stream audio to Deepgram successfully.

<Warning>
  If you're already confident you can stream audio to Deepgram and receive transcriptions, you can skip to [3. Streaming Your Audio](#3-streaming-other-audio).
</Warning>

## Stream a File

While streaming a file isn't our recommended way to use Deepgram's real-time transcription service (we suggest our [pre-recorded API](/docs/pre-recorded-audio) for that), it's a quick and easy way to make sure your API key and network are functioning correctly.

Just run the following command:

`python test_suite.py -k YOUR_DEEPGRAM_API_KEY`

You may need to use the command `python3` instead.

<Warning>
  Make sure to replace `YOUR_DEEPGRAM_API_KEY` with an API key generated from our [Console](https://console.deepgram.com/).
</Warning>

This will stream the included file, `preamble.wav`, to Deepgram and print out transcripts to your terminal.

You can also stream your own WAV file by running:

`python test_suite.py -k YOUR_DEEPGRAM_API_KEY -i /path/to/audio.wav`

To check out how this functionality is implemented, look at the conditional `elif method == 'wav'` in our `sender` function.

<Info>
  Self-hosting a Deepgram deployment? You can provide your custom URL to the test suite with the `--host` argument.
</Info>

## Stream Your Microphone

The starter kit also has the ability to send audio from your microphone to Deepgram for transcription.

First, make sure [pyaudio](https://pypi.org/project/PyAudio/) and its [portaudio](http://portaudio.com/) dependency are installed, and you have a microphone connected to your computer. Then, run:

`python test_suite.py -k YOUR_DEEPGRAM_API_KEY -i mic`

## Additional Options

The following arguments can be appended to any test suite command.

### Parameters

`--model/-m`: Specify a Deepgram model. Example: `--model phonecall`. Defaults to `general`.

### Timestamps

`--timestamps/-ts`: Opt-in to printing start and end timestamps in seconds for each streaming response. Example: `--timestamps`

Sample output line with timestamps:

```
In order to form a more perfect union, [2.5 - 4.26]
```

### Subtitle Generation

In addition to printing transcripts to the terminal, the test suite can also wrap Deepgram's responses in two common subtitle formats, SRT or VTT.

To generate SRT or VTT files, add the `-f/--format` parameter when running the test suite:

`python test_suite.py -k YOUR_DEEPGRAM_API_KEY [-i mic|/path/to/audio.wav] [-f text|vtt|srt]`

This parameter defaults to `text`, which outputs responses to your terminal.

***

If you were able to successfully stream local audio and receive a transcript, you're ready to move on to the next step!

# Streaming a Remote Source

The next step in getting started with Deepgram's audio streaming capabilities is to learn how to stream a remote file to Deepgram. This task introduces slightly more complexity and requires managing multiple asynchronous remote sources—one for audio input to Deepgram, one for Deepgram's transcription output.

## Stream a URL

Make sure you have the URL for direct audio stream to test with. A good way of testing this is to open the URL in a browser—you should see just the built-in browser audio player without an accompanying web page.

Here are two URLs for you to try:

* BBC World Service: [http://stream.live.vc.bbcmedia.co.uk/bbc\_world\_service](http://stream.live.vc.bbcmedia.co.uk/bbc_world_service)
* France Inter: [https://direct.franceinter.fr/live/franceinter-midfi.mp3](https://direct.franceinter.fr/live/franceinter-midfi.mp3)

If you use the French channel, be sure to add `language=fr` to your Deepgram URL.

Then, run the test suite to see the results:

`python test_suite.py -k YOUR_DEEPGRAM_API_KEY -i http://stream.live.vc.bbcmedia.co.uk/bbc_world_service`

To check out how this functionality is implemented, look at the conditional `elif method == url` in our `sender` function. We use the `aiohttp` library to make an asynchronous request and open a session, then send content to Deepgram.

# Streaming Other Audio

Now that you've validated you can stream WAV files and URLs to Deepgram, it's time to start the process of integrating other audio sources, so you can build something with Deepgram that's tailored to your business needs. To do this, we'll start by taking a step back…and removing Deepgram from the picture!

Let's set the `test_suite.py` file aside for the moment. In addition to that file, the test suite also comes with a mock server and client: `server.py` and `client.py`. These are intended to create the simplest possible environment to test your custom audio.

The mock server exposes a similar interface to Deepgram's streaming service. It accepts websocket connections that specify an encoding, sample rate, and number of channels; and it expects a stream of raw audio. However, it doesn't transcribe that audio. All it does is send back messages confirming how much audio data has been received, and once the client closes the stream, it saves all sent audio to a file.

Using the mock server for testing allows you to focus on getting your audio and client code right. Once you're confident that your audio stream is configured correctly and you're streaming the audio you expect, you can easily swap to sending that audio to Deepgram's service.

## Run the Mock Server

Start by running the mock server:

`python server.py`

Then, open another terminal window and prepare to run the mock client.

The mock client accepts these parameters:

`python client.py [-i INPUT] [-e ENCODING] [-s SAMPLE_RATE] [-c CHANNELS]`

The starter kit comes with a raw audio file, `preamble.raw` , that you can use to test streaming to the mock server. You can stream `preamble.raw` with the mock client like so:

`python client.py -i preamble.raw -e linear16 -s 8000 -c 1`

When you run the mock client, you should see output confirming that the mock server has begun to receive your audio.

![](file:f1b5c5ca-23aa-4e13-b406-4fc2de08efdb)

For a list of valid encodings, see [our endcoding documentation](/docs/encoding/).

## Validate Your Audio

At the end of an audio stream, the mock server saves all audio data that was sent in a RAW file. It will return the filename to you at the end of the stream.

![image of terminal showing message that websocket is receiving data](file:de0b8546-175f-49d8-8cf0-fde1a8a56b5b)

You need to ensure the audio the server received is the audio you intended to send. To validate this, open this file in a program like Audacity (specifying necessary parameters like the encoding and sample rate) and try to play it back. You should be able to listen to your audio and verify it's correct.

## Stream to Deepgram

Once you verify your audio is correct, you can try streaming that audio to Deepgram. To do so, simply swap the websocket URL in `client.py` to point to Deepgram—the correct URL is left in a comment for you.

![image of terminal showing lines to edit to connect to Deepgram](file:77b88680-b1b2-4f19-a47f-688631368d75)

Don't forget add your DG API key to the websocket headers where it says `YOUR_DG_API_KEY`.

![](file:3cf3ee72-4031-458b-91e2-c0b45f3e5a3b)

If you were able to stream to the mock server, and have validated your audio sounds correct, you should be able to seamlessly start receiving transcriptions from Deepgram.

# Wrap-Up

By following the starter kit steps, you've built your knowledge of working with websockets, audio, and Deepgram's system. We hope this guide has enabled you to build your own custom audio integrations with confidence.

***




***

# Live Audio

GET /v1/listen

Transcribe audio and video using Deepgram's speech-to-text WebSocket

Reference: https://developers.deepgram.com/reference/speech-to-text/listen-streaming

## AsyncAPI Specification

```yaml
asyncapi: 2.6.0
info:
  title: listen.v1
  version: subpackage_listen/v1.listen.v1
  description: Transcribe audio and video using Deepgram's speech-to-text WebSocket
channels:
  /v1/listen:
    description: Transcribe audio and video using Deepgram's speech-to-text WebSocket
    bindings:
      ws:
        query:
          type: object
          properties:
            callback:
              $ref: '#/components/schemas/ListenV1Callback'
            callback_method:
              $ref: '#/components/schemas/ListenV1CallbackMethod'
            channels:
              $ref: '#/components/schemas/ListenV1Channels'
            detect_entities:
              $ref: '#/components/schemas/ListenV1DetectEntities'
            diarize:
              $ref: '#/components/schemas/ListenV1Diarize'
            dictation:
              $ref: '#/components/schemas/ListenV1Dictation'
            encoding:
              $ref: '#/components/schemas/ListenV1Encoding'
            endpointing:
              $ref: '#/components/schemas/ListenV1Endpointing'
            extra:
              $ref: '#/components/schemas/ListenV1Extra'
            interim_results:
              $ref: '#/components/schemas/ListenV1InterimResults'
            keyterm:
              $ref: '#/components/schemas/ListenV1Keyterm'
            keywords:
              $ref: '#/components/schemas/ListenV1Keywords'
            language:
              $ref: '#/components/schemas/ListenV1Language'
            mip_opt_out:
              $ref: '#/components/schemas/ListenV1MipOptOut'
            model:
              $ref: '#/components/schemas/ListenV1Model'
            multichannel:
              $ref: '#/components/schemas/ListenV1Multichannel'
            numerals:
              $ref: '#/components/schemas/ListenV1Numerals'
            profanity_filter:
              $ref: '#/components/schemas/ListenV1ProfanityFilter'
            punctuate:
              $ref: '#/components/schemas/ListenV1Punctuate'
            redact:
              $ref: '#/components/schemas/ListenV1Redact'
            replace:
              $ref: '#/components/schemas/ListenV1Replace'
            sample_rate:
              $ref: '#/components/schemas/ListenV1SampleRate'
            search:
              $ref: '#/components/schemas/ListenV1Search'
            smart_format:
              $ref: '#/components/schemas/ListenV1SmartFormat'
            tag:
              $ref: '#/components/schemas/ListenV1Tag'
            utterance_end_ms:
              $ref: '#/components/schemas/ListenV1UtteranceEndMs'
            vad_events:
              $ref: '#/components/schemas/ListenV1VadEvents'
            version:
              $ref: '#/components/schemas/ListenV1Version'
        headers:
          type: object
          properties:
            Authorization:
              type: string
    publish:
      operationId: listen-v-1-publish
      summary: Server messages
      message:
        oneOf:
          - $ref: >-
              #/components/messages/subpackage_listen/v1.listen.v1-server-0-ListenV1Results
          - $ref: >-
              #/components/messages/subpackage_listen/v1.listen.v1-server-1-ListenV1Metadata
          - $ref: >-
              #/components/messages/subpackage_listen/v1.listen.v1-server-2-ListenV1UtteranceEnd
          - $ref: >-
              #/components/messages/subpackage_listen/v1.listen.v1-server-3-ListenV1SpeechStarted
    subscribe:
      operationId: listen-v-1-subscribe
      summary: Client messages
      message:
        oneOf:
          - $ref: >-
              #/components/messages/subpackage_listen/v1.listen.v1-client-0-ListenV1Media
          - $ref: >-
              #/components/messages/subpackage_listen/v1.listen.v1-client-1-ListenV1Finalize
          - $ref: >-
              #/components/messages/subpackage_listen/v1.listen.v1-client-2-ListenV1CloseStream
          - $ref: >-
              #/components/messages/subpackage_listen/v1.listen.v1-client-3-ListenV1KeepAlive
servers:
  Production:
    url: wss://api.deepgram.com/
    protocol: wss
    x-default: true
  Agent:
    url: wss://api.deepgram.com/
    protocol: wss
components:
  messages:
    subpackage_listen/v1.listen.v1-server-0-ListenV1Results:
      name: ListenV1Results
      title: ListenV1Results
      description: Receive transcription results
      payload:
        $ref: '#/components/schemas/ListenV1_ListenV1Results'
    subpackage_listen/v1.listen.v1-server-1-ListenV1Metadata:
      name: ListenV1Metadata
      title: ListenV1Metadata
      description: Receive metadata about the transcription
      payload:
        $ref: '#/components/schemas/ListenV1_ListenV1Metadata'
    subpackage_listen/v1.listen.v1-server-2-ListenV1UtteranceEnd:
      name: ListenV1UtteranceEnd
      title: ListenV1UtteranceEnd
      description: Receive an utterance end event
      payload:
        $ref: '#/components/schemas/ListenV1_ListenV1UtteranceEnd'
    subpackage_listen/v1.listen.v1-server-3-ListenV1SpeechStarted:
      name: ListenV1SpeechStarted
      title: ListenV1SpeechStarted
      description: Receive a speech started event
      payload:
        $ref: '#/components/schemas/ListenV1_ListenV1SpeechStarted'
    subpackage_listen/v1.listen.v1-client-0-ListenV1Media:
      name: ListenV1Media
      title: ListenV1Media
      description: Send audio or video data to be transcribed
      payload:
        $ref: '#/components/schemas/ListenV1_ListenV1Media'
    subpackage_listen/v1.listen.v1-client-1-ListenV1Finalize:
      name: ListenV1Finalize
      title: ListenV1Finalize
      description: Send a Finalize message to flush the WebSocket stream
      payload:
        $ref: '#/components/schemas/ListenV1_ListenV1Finalize'
    subpackage_listen/v1.listen.v1-client-2-ListenV1CloseStream:
      name: ListenV1CloseStream
      title: ListenV1CloseStream
      description: Send a CloseStream message to close the WebSocket stream
      payload:
        $ref: '#/components/schemas/ListenV1_ListenV1CloseStream'
    subpackage_listen/v1.listen.v1-client-3-ListenV1KeepAlive:
      name: ListenV1KeepAlive
      title: ListenV1KeepAlive
      description: Send a KeepAlive message to keep the WebSocket stream alive
      payload:
        $ref: '#/components/schemas/ListenV1_ListenV1KeepAlive'
  schemas:
    ListenV1Callback:
      description: Any type
      title: ListenV1Callback
    ListenV1CallbackMethod:
      type: string
      enum:
        - POST
        - GET
        - PUT
        - DELETE
      default: POST
      description: HTTP method by which the callback request will be made
      title: ListenV1CallbackMethod
    ListenV1Channels:
      description: Any type
      title: ListenV1Channels
    ListenV1DetectEntities:
      type: string
      enum:
        - 'true'
        - 'false'
      default: 'false'
      description: >-
        Identifies and extracts key entities from content in submitted audio.
        Entities appear in final results. When enabled, Punctuation will also be
        enabled by default
      title: ListenV1DetectEntities
    ListenV1Diarize:
      type: string
      enum:
        - 'true'
        - 'false'
      default: 'false'
      description: >-
        Defaults to `false`. Recognize speaker changes. Each word in the
        transcript will be assigned a speaker number starting at 0
      title: ListenV1Diarize
    ListenV1Dictation:
      type: string
      enum:
        - 'true'
        - 'false'
      default: 'false'
      description: Identify and extract key entities from content in submitted audio
      title: ListenV1Dictation
    ListenV1Encoding:
      type: string
      enum:
        - linear16
        - linear32
        - flac
        - alaw
        - mulaw
        - amr-nb
        - amr-wb
        - opus
        - ogg-opus
        - speex
        - g729
      description: Specify the expected encoding of your submitted audio
      title: ListenV1Encoding
    ListenV1Endpointing:
      description: Any type
      title: ListenV1Endpointing
    ListenV1Extra:
      description: Any type
      title: ListenV1Extra
    ListenV1InterimResults:
      type: string
      enum:
        - 'true'
        - 'false'
      default: 'false'
      description: >-
        Specifies whether the streaming endpoint should provide ongoing
        transcription updates as more audio is received. When set to true, the
        endpoint sends continuous updates, meaning transcription results may
        evolve over time
      title: ListenV1InterimResults
    ListenV1Keyterm:
      description: Any type
      title: ListenV1Keyterm
    ListenV1Keywords:
      description: Any type
      title: ListenV1Keywords
    ListenV1Language:
      description: Any type
      title: ListenV1Language
    ListenV1MipOptOut:
      description: Any type
      title: ListenV1MipOptOut
    ListenV1Model:
      type: string
      enum:
        - nova-3
        - nova-3-general
        - nova-3-medical
        - nova-2
        - nova-2-general
        - nova-2-meeting
        - nova-2-finance
        - nova-2-conversationalai
        - nova-2-voicemail
        - nova-2-video
        - nova-2-medical
        - nova-2-drivethru
        - nova-2-automotive
        - nova
        - nova-general
        - nova-phonecall
        - nova-medical
        - enhanced
        - enhanced-general
        - enhanced-meeting
        - enhanced-phonecall
        - enhanced-finance
        - base
        - meeting
        - phonecall
        - finance
        - conversationalai
        - voicemail
        - video
        - custom
      description: AI model to use for the transcription
      title: ListenV1Model
    ListenV1Multichannel:
      type: string
      enum:
        - 'true'
        - 'false'
      default: 'false'
      description: Transcribe each audio channel independently
      title: ListenV1Multichannel
    ListenV1Numerals:
      type: string
      enum:
        - 'true'
        - 'false'
      default: 'false'
      description: Convert numbers from written format to numerical format
      title: ListenV1Numerals
    ListenV1ProfanityFilter:
      type: string
      enum:
        - 'true'
        - 'false'
      default: 'false'
      description: >-
        Profanity Filter looks for recognized profanity and converts it to the
        nearest recognized non-profane word or removes it from the transcript
        completely
      title: ListenV1ProfanityFilter
    ListenV1Punctuate:
      type: string
      enum:
        - 'true'
        - 'false'
      default: 'false'
      description: Add punctuation and capitalization to the transcript
      title: ListenV1Punctuate
    ListenV1Redact:
      type: string
      enum:
        - 'true'
        - 'false'
        - pci
        - numbers
        - aggressive_numbers
        - ssn
      default: 'false'
      description: Redaction removes sensitive information from your transcripts
      title: ListenV1Redact
    ListenV1Replace:
      description: Any type
      title: ListenV1Replace
    ListenV1SampleRate:
      description: Any type
      title: ListenV1SampleRate
    ListenV1Search:
      description: Any type
      title: ListenV1Search
    ListenV1SmartFormat:
      type: string
      enum:
        - 'true'
        - 'false'
      default: 'false'
      description: >-
        Apply formatting to transcript output. When set to true, additional
        formatting will be applied to transcripts to improve readability
      title: ListenV1SmartFormat
    ListenV1Tag:
      description: Any type
      title: ListenV1Tag
    ListenV1UtteranceEndMs:
      description: Any type
      title: ListenV1UtteranceEndMs
    ListenV1VadEvents:
      type: string
      enum:
        - 'true'
        - 'false'
      default: 'false'
      description: >-
        Indicates that speech has started. You'll begin receiving Speech Started
        messages upon speech starting
      title: ListenV1VadEvents
    ListenV1Version:
      description: Any type
      title: ListenV1Version
    ChannelsListenV1MessagesListenV1ResultsType:
      type: string
      enum:
        - Results
      description: Message type identifier
      title: ChannelsListenV1MessagesListenV1ResultsType
    ChannelsListenV1MessagesListenV1ResultsChannelAlternativesItemsWordsItems:
      type: object
      properties:
        word:
          type: string
          description: The word of the transcription
        start:
          type: number
          format: double
          description: The start time of the word
        end:
          type: number
          format: double
          description: The end time of the word
        confidence:
          type: number
          format: double
          description: The confidence of the word
        language:
          type: string
          description: The language of the word
        punctuated_word:
          type: string
          description: The punctuated word of the word
        speaker:
          type: number
          format: double
          description: The speaker of the word
      required:
        - word
        - start
        - end
        - confidence
      title: >-
        ChannelsListenV1MessagesListenV1ResultsChannelAlternativesItemsWordsItems
    ChannelsListenV1MessagesListenV1ResultsChannelAlternativesItems:
      type: object
      properties:
        transcript:
          type: string
          description: The transcript of the transcription
        confidence:
          type: number
          format: double
          description: The confidence of the transcription
        languages:
          type: array
          items:
            type: string
        words:
          type: array
          items:
            $ref: >-
              #/components/schemas/ChannelsListenV1MessagesListenV1ResultsChannelAlternativesItemsWordsItems
      required:
        - transcript
        - confidence
        - words
      title: ChannelsListenV1MessagesListenV1ResultsChannelAlternativesItems
    ChannelsListenV1MessagesListenV1ResultsChannel:
      type: object
      properties:
        alternatives:
          type: array
          items:
            $ref: >-
              #/components/schemas/ChannelsListenV1MessagesListenV1ResultsChannelAlternativesItems
      required:
        - alternatives
      title: ChannelsListenV1MessagesListenV1ResultsChannel
    ChannelsListenV1MessagesListenV1ResultsMetadataModelInfo:
      type: object
      properties:
        name:
          type: string
          description: The name of the model
        version:
          type: string
          description: The version of the model
        arch:
          type: string
          description: The arch of the model
      required:
        - name
        - version
        - arch
      title: ChannelsListenV1MessagesListenV1ResultsMetadataModelInfo
    ChannelsListenV1MessagesListenV1ResultsMetadata:
      type: object
      properties:
        request_id:
          type: string
          description: The request ID
        model_info:
          $ref: >-
            #/components/schemas/ChannelsListenV1MessagesListenV1ResultsMetadataModelInfo
        model_uuid:
          type: string
          description: The model UUID
      required:
        - request_id
        - model_info
        - model_uuid
      title: ChannelsListenV1MessagesListenV1ResultsMetadata
    ChannelsListenV1MessagesListenV1ResultsEntitiesItems:
      type: object
      properties:
        label:
          type: string
          description: >-
            The type/category of the entity (e.g., NAME, PHONE_NUMBER,
            EMAIL_ADDRESS, ORGANIZATION, CARDINAL)
        value:
          type: string
          description: The formatted text representation of the entity
        raw_value:
          type: string
          description: >-
            The original spoken text of the entity (present when formatting is
            enabled)
        confidence:
          type: number
          format: double
          description: The confidence score of the entity detection
        start_word:
          type: integer
          description: >-
            The index of the first word of the entity in the transcript
            (inclusive)
        end_word:
          type: integer
          description: >-
            The index of the last word of the entity in the transcript
            (exclusive)
      required:
        - label
        - value
        - raw_value
        - confidence
        - start_word
        - end_word
      title: ChannelsListenV1MessagesListenV1ResultsEntitiesItems
    ListenV1_ListenV1Results:
      type: object
      properties:
        type:
          $ref: '#/components/schemas/ChannelsListenV1MessagesListenV1ResultsType'
          description: Message type identifier
        channel_index:
          type: array
          items:
            type: number
            format: double
          description: The index of the channel
        duration:
          type: number
          format: double
          description: The duration of the transcription
        start:
          type: number
          format: double
          description: The start time of the transcription
        is_final:
          type: boolean
          description: Whether the transcription is final
        speech_final:
          type: boolean
          description: Whether the transcription is speech final
        channel:
          $ref: '#/components/schemas/ChannelsListenV1MessagesListenV1ResultsChannel'
        metadata:
          $ref: '#/components/schemas/ChannelsListenV1MessagesListenV1ResultsMetadata'
        from_finalize:
          type: boolean
          description: Whether the transcription is from a finalize message
        entities:
          type: array
          items:
            $ref: >-
              #/components/schemas/ChannelsListenV1MessagesListenV1ResultsEntitiesItems
          description: >-
            Extracted entities from the audio when detect_entities is enabled.
            Only present in is_final messages. Returns an empty array if no
            entities are detected
      required:
        - type
        - channel_index
        - duration
        - start
        - channel
        - metadata
      title: ListenV1_ListenV1Results
    ChannelsListenV1MessagesListenV1MetadataType:
      type: string
      enum:
        - Metadata
      description: Message type identifier
      title: ChannelsListenV1MessagesListenV1MetadataType
    ListenV1_ListenV1Metadata:
      type: object
      properties:
        type:
          $ref: '#/components/schemas/ChannelsListenV1MessagesListenV1MetadataType'
          description: Message type identifier
        transaction_key:
          type: string
          description: The transaction key
        request_id:
          type: string
          format: uuid
          description: The request ID
        sha256:
          type: string
          description: The sha256
        created:
          type: string
          description: The created
        duration:
          type: number
          format: double
          description: The duration
        channels:
          type: number
          format: double
          description: The channels
      required:
        - type
        - transaction_key
        - request_id
        - sha256
        - created
        - duration
        - channels
      title: ListenV1_ListenV1Metadata
    ChannelsListenV1MessagesListenV1UtteranceEndType:
      type: string
      enum:
        - UtteranceEnd
      description: Message type identifier
      title: ChannelsListenV1MessagesListenV1UtteranceEndType
    ListenV1_ListenV1UtteranceEnd:
      type: object
      properties:
        type:
          $ref: >-
            #/components/schemas/ChannelsListenV1MessagesListenV1UtteranceEndType
          description: Message type identifier
        channel:
          type: array
          items:
            type: number
            format: double
          description: The channel
        last_word_end:
          type: number
          format: double
          description: The last word end
      required:
        - type
        - channel
        - last_word_end
      title: ListenV1_ListenV1UtteranceEnd
    ChannelsListenV1MessagesListenV1SpeechStartedType:
      type: string
      enum:
        - SpeechStarted
      description: Message type identifier
      title: ChannelsListenV1MessagesListenV1SpeechStartedType
    ListenV1_ListenV1SpeechStarted:
      type: object
      properties:
        type:
          $ref: >-
            #/components/schemas/ChannelsListenV1MessagesListenV1SpeechStartedType
          description: Message type identifier
        channel:
          type: array
          items:
            type: number
            format: double
          description: The channel
        timestamp:
          type: number
          format: double
          description: The timestamp
      required:
        - type
        - channel
        - timestamp
      title: ListenV1_ListenV1SpeechStarted
    ListenV1_ListenV1Media:
      type: string
      format: binary
      title: ListenV1_ListenV1Media
    ChannelsListenV1MessagesListenV1FinalizeType:
      type: string
      enum:
        - Finalize
        - CloseStream
        - KeepAlive
      description: Message type identifier
      title: ChannelsListenV1MessagesListenV1FinalizeType
    ListenV1_ListenV1Finalize:
      type: object
      properties:
        type:
          $ref: '#/components/schemas/ChannelsListenV1MessagesListenV1FinalizeType'
          description: Message type identifier
      required:
        - type
      title: ListenV1_ListenV1Finalize
    ChannelsListenV1MessagesListenV1CloseStreamType:
      type: string
      enum:
        - Finalize
        - CloseStream
        - KeepAlive
      description: Message type identifier
      title: ChannelsListenV1MessagesListenV1CloseStreamType
    ListenV1_ListenV1CloseStream:
      type: object
      properties:
        type:
          $ref: '#/components/schemas/ChannelsListenV1MessagesListenV1CloseStreamType'
          description: Message type identifier
      required:
        - type
      title: ListenV1_ListenV1CloseStream
    ChannelsListenV1MessagesListenV1KeepAliveType:
      type: string
      enum:
        - Finalize
        - CloseStream
        - KeepAlive
      description: Message type identifier
      title: ChannelsListenV1MessagesListenV1KeepAliveType
    ListenV1_ListenV1KeepAlive:
      type: object
      properties:
        type:
          $ref: '#/components/schemas/ChannelsListenV1MessagesListenV1KeepAliveType'
          description: Message type identifier
      required:
        - type
      title: ListenV1_ListenV1KeepAlive

```



title: Getting Started
subtitle: >-
An introduction to getting transcription data from live streaming audio in
real time.
slug: docs/live-streaming-audio
-------------------------------

<Card href="https://playground.deepgram.com/?endpoint=listen-streaming&language=en&model=nova-3">
  <div class="t-default text-base font-semibold">Deepgram API Playground</div>
  Try this feature out in our API Playground.
</Card>

In this guide, you'll learn how to automatically transcribe live streaming audio in real time using Deepgram's SDKs, which are supported for use with the [Deepgram API](/reference/deepgram-api-overview). (If you prefer not to use a Deepgram SDK, jump to the section [Non-SDK Code Examples](/docs/live-streaming-audio#non-sdk-code-examples).)

<Info>
  Before you start, you'll need to follow the steps in the [Make Your First API Request](/guides/fundamentals/make-your-first-api-request) guide to obtain a Deepgram API key, and configure your environment if you are choosing to use a Deepgram SDK.
</Info>

## SDKs

To transcribe audio from an audio stream using one of Deepgram's SDKs, follow these steps.

### Install the SDK

Open your terminal, navigate to the location on your drive where you want to create your project, and install the Deepgram SDK.

<CodeGroup>
  ```JavaScript
  // Install the Deepgram JS SDK
  // https://github.com/deepgram/deepgram-js-sdk

  // npm install @deepgram/sdk
  ```

  ```Python
  # Install the Deepgram Python SDK
  # https://github.com/deepgram/deepgram-python-sdk

  # pip install deepgram-sdk
  ```

  ```csharp C#
  // Install the Deepgram .NET SDK
  // https://github.com/deepgram/deepgram-dotnet-sdk

  // dotnet add package Deepgram
  ```

  ```Go
  // Install the Deepgram Go SDK
  // https://github.com/deepgram/deepgram-go-sdk

  // go get github.com/deepgram/deepgram-go-sdk
  ```
</CodeGroup>

### Add Dependencies

<CodeGroup>
  ```JavaScript
  // Install cross-fetch: Platform-agnostic Fetch API with typescript support, a simple interface, and optional polyfill.
  // Install dotenv to protect your api key

  // $ npm install cross-fetch dotenv
  ```

  ```Python
  # Install httpx to make http requests

  # pip install httpx
  ```

  ```csharp C#
  // In your .csproj file, add the Package Reference:

  // <ItemGroup>
  //     <PackageReference Include="Deepgram" Version="4.4.0" />
  // </ItemGroup>
  ```

  ```Go
  // Importing the Deepgram Go SDK should pull in all dependencies required
  ```
</CodeGroup>

### Transcribe Audio from a Remote Stream

The following code shows how to transcribe audio from a remote audio stream.

<CodeGroup>
  ```javascript JavaScript
  // Example filename: index.js

  const { DeepgramClient } = require("@deepgram/sdk");
  const fetch = require("cross-fetch");
  const dotenv = require("dotenv");
  dotenv.config();

  // URL for the realtime streaming audio you would like to transcribe
  const url = "http://stream.live.vc.bbcmedia.co.uk/bbc_world_service";

  const live = async () => {
    // STEP 1: Create a Deepgram client using the API key
    const deepgram = new DeepgramClient({ apiKey: process.env.DEEPGRAM_API_KEY });

    // STEP 2: Create a live transcription connection
    const connection = await deepgram.listen.v1.connect({
      model: "nova-3",
      language: "en-US",
      smart_format: "true",
    });

    // STEP 3: Listen for events from the live transcription connection
    connection.on("open", () => {
      connection.on("close", () => {
        console.log("Connection closed.");
      });

      connection.on("message", (data) => {
        if (data.type === "Results") {
          console.log(data.channel.alternatives[0].transcript);
        }
      });

      connection.on("error", (err) => {
        console.error(err);
      });

      // STEP 4: Fetch the audio stream and send it to the live transcription connection
      fetch(url)
        .then((r) => r.body)
        .then((res) => {
          res.on("readable", () => {
            connection.sendMedia(res.read());
          });
        });
    });

    connection.connect();
    await connection.waitForOpen();
  };

  live();
  ```

  ```python Python
  # Example filename: main.py

  # For more Python SDK migration guides, visit:
  # https://github.com/deepgram/deepgram-python-sdk/tree/main/docs

  # Set your Deepgram API key as an environment variable:
  # export DEEPGRAM_API_KEY="your-api-key-here"

  import httpx
  import logging
  import threading

  from deepgram import (
      DeepgramClient,
  )
  from deepgram.core.events import EventType

  # URL for the realtime streaming audio you would like to transcribe
  URL = "http://stream.live.vc.bbcmedia.co.uk/bbc_world_service"

  def main():
      try:
          # use default config
          deepgram: DeepgramClient = DeepgramClient()

          # Create a websocket connection to Deepgram
          with deepgram.listen.v1.connect(model="nova-3") as connection:
              def on_message(message) -> None:
                  msg_type = getattr(message, "type", "Unknown")
                  if hasattr(message, 'channel') and hasattr(message.channel, 'alternatives'):
                      sentence = message.channel.alternatives[0].transcript
                      if len(sentence) == 0:
                          return
                      print(message.channel.json(indent=4))

              connection.on(EventType.OPEN, lambda _: print("Connection opened"))
              connection.on(EventType.MESSAGE, on_message)
              connection.on(EventType.CLOSE, lambda _: print("Connection closed"))
              connection.on(EventType.ERROR, lambda error: print(f"Error: {error}"))

              lock_exit = threading.Lock()
              exit = False

              # Define a thread for start_listening with error handling
              def listening_thread():
                  try:
                      connection.start_listening()
                  except Exception as e:
                      print(f"Error in listening thread: {e}")

              # Start listening in a separate thread
              listen_thread = threading.Thread(target=listening_thread)
              listen_thread.start()

              # define a worker thread for HTTP streaming with error handling
              def myThread():
                  try:
                      with httpx.stream("GET", URL) as r:
                          for data in r.iter_bytes():
                              lock_exit.acquire()
                              if exit:
                                  break
                              lock_exit.release()

                              connection.send_media(data)
                  except Exception as e:
                      print(f"Error in HTTP streaming thread: {e}")

              # start the HTTP streaming thread
              myHttp = threading.Thread(target=myThread)
              myHttp.start()

              # signal finished
              input("")
              lock_exit.acquire()
              exit = True
              lock_exit.release()

              # Wait for both threads to close and join with timeout
              myHttp.join(timeout=5.0)
              listen_thread.join(timeout=5.0)

              print("Finished")

      except Exception as e:
          print(f"Could not open socket: {e}")
          return

  if __name__ == "__main__":
      main()
  ```

  ```csharp C#
  // Example filename: Program.cs

  using Deepgram.Models.Listen.v2.WebSocket;

  namespace SampleApp
  {
      class Program
      {
          static async Task Main(string[] args)
          {
              try
              {
                  // Initialize Library with default logging
                  Library.Initialize();

                  // use the client factory with a API Key set with the "DEEPGRAM_API_KEY" environment variable
                  var liveClient = new ListenWebSocketClient();

                  // Subscribe to the EventResponseReceived event
                  await liveClient.Subscribe(new EventHandler<ResultResponse>((sender, e) =>
                  {
                      if (e.Channel.Alternatives[0].Transcript == "")
                      {
                          return;
                      }
                      Console.WriteLine($"Speaker: {e.Channel.Alternatives[0].Transcript}");
                  }));

                  // Start the connection
                  var liveSchema = new LiveSchema()
                  {
                      Model = "nova-3",
                      SmartFormat = true,
                  };
                  bool bConnected = await liveClient.Connect(liveSchema);
                  if (!bConnected)
                  {
                      Console.WriteLine("Failed to connect to the server");
                      return;
                  }

                  // get the webcast data... this is a blocking operation
                  try
                  {
                      var url = "http://stream.live.vc.bbcmedia.co.uk/bbc_world_service";
                      using (HttpClient client = new HttpClient())
                      {
                          using (Stream receiveStream = await client.GetStreamAsync(url))
                          {
                              while (liveClient.IsConnected())
                              {
                                  byte[] buffer = new byte[2048];
                                  await receiveStream.ReadAsync(buffer, 0, buffer.Length);
                                  liveClient.Send(buffer);
                              }
                          }
                      }
                  }
                  catch (Exception e)
                  {
                      Console.WriteLine(e.Message);
                  }

                  // Stop the connection
                  await liveClient.Stop();

                  // Teardown Library
                  Library.Terminate();
              }
              catch (Exception e)
              {
                  Console.WriteLine(e.Message);
              }
          }
      }
  }
  ```

  ```go Go
  // Example filename: main.go
  package main

  import (
  	"bufio"
  	"context"
  	"fmt"
  	"net/http"
  	"os"
  	"reflect"

  	interfaces "github.com/deepgram/deepgram-go-sdk/pkg/client/interfaces"
  	client "github.com/deepgram/deepgram-go-sdk/pkg/client/live"
  )

  const (
  	STREAM_URL = "http://stream.live.vc.bbcmedia.co.uk/bbc_world_service"
  )

  func main() {
  	// STEP 1: init Deepgram client library
  	client.InitWithDefault()

  	// STEP 2: define context to manage the lifecycle of the request
  	ctx := context.Background()

  	// STEP 3: define options for the request
  	transcriptOptions := interfaces.LiveTranscriptionOptions{
  		Model:       "nova-3",
  		Language:    "en-US",
  		SmartFormat: true,
  	}

  	// STEP 4: create a Deepgram client using default settings
  	// NOTE: you can set your API KEY in your bash profile by typing the following line in your shell:
  	// export DEEPGRAM_API_KEY = "YOUR_DEEPGRAM_API_KEY"
  	dgClient, err := client.NewForDemo(ctx, &transcriptOptions)
  	if err != nil {
  		fmt.Println("ERROR creating LiveTranscription connection:", err)
  		return
  	}

  	// STEP 5: connect to the Deepgram service
  	bConnected := dgClient.Connect()
  	if !bConnected {
  		fmt.Println("Client.Connect failed")
  		os.Exit(1)
  	}

  	// STEP 6: create an HTTP client to stream audio data
  	httpClient := new(http.Client)

  	// STEP 7: create an HTTP stream
  	res, err := httpClient.Get(STREAM_URL)
  	if err != nil {
  		fmt.Printf("httpClient.Get failed. Err: %v\n", err)
  		return
  	}

  	fmt.Printf("Stream is up and running %s\n", reflect.TypeOf(res))

  	go func() {
  		// STEP 8: feed the HTTP stream to the Deepgram client (this is a blocking call)
  		dgClient.Stream(bufio.NewReader(res.Body))
  	}()

  	// STEP 9: wait for user to exit
  	fmt.Print("Press ENTER to exit!\n\n")
  	input := bufio.NewScanner(os.Stdin)
  	input.Scan()

  	// STEP 10: close HTTP stream
  	res.Body.Close()

  	// STEP 11: close the Deepgram client
  	dgClient.Stop()

  	fmt.Printf("Program exiting...\n")
  }
  ```
</CodeGroup>

<Info>
  The above example includes the parameter `model=nova-3`, which tells the API to use Deepgram's latest model. Removing this parameter will result in the API using the default model, which is currently `model=base`.

  It also includes Deepgram's [Smart Formatting](/docs/smart-format) feature, `smart_format=true`. This will format currency amounts, phone numbers, email addresses, and more for enhanced transcript readability.
</Info>

## Non-SDK Code Examples

If you would like to try out making a Deepgram speech-to-text request in a specific language (but not using Deepgram's SDKs), we offer a library of code-samples in this [Github repo](https://github.com/deepgram-devs/code-samples). However, we recommend first trying out our SDKs.

## Results

In order to see the results from Deepgram, you must run the application. Run your application from the terminal. Your transcripts will appear in your shell.

<CodeGroup>
  ```javascript JavaScript
  # Run your application using the file you created in the previous step
  # Example: node index.js

  node YOUR_FILE_NAME.js
  ```

  ```shell Python
  # Run your application using the file you created in the previous step
  # Example: python main.py

  python YOUR_FILE_NAME.py
  ```

  ```shell C#
  # Run your application using the file you created in the previous step
  # Example: dotnet run Program.cs

  dotnet run YOUR_FILE_NAME.cs
  ```

  ```shell Go
  # Run your application using the file you created in the previous step
  # Example: go run main.go

  go run YOUR_FILE_NAME.go
  ```
</CodeGroup>

<Warning>
  Deepgram does not store transcripts, so the Deepgram API response is the only opportunity to retrieve the transcript. Make sure to save output or [return transcriptions to a callback URL for custom processing](/docs/callback/).
</Warning>

### Analyze the Response

The responses that are returned will look similar to this:

<CodeGroup>
  ```json JSON
  {
    "type": "Results",
    "channel_index": [
      0,
      1
    ],
    "duration": 1.98,
    "start": 5.99,
    "is_final": true,
    "speech_final": true,
    "channel": {
      "alternatives": [
        {
          "transcript": "Tell me more about this.",
          "confidence": 0.99964225,
          "words": [
            {
              "word": "tell",
              "start": 6.0699997,
              "end": 6.3499994,
              "confidence": 0.99782443,
              "punctuated_word": "Tell"
            },
            {
              "word": "me",
              "start": 6.3499994,
              "end": 6.6299996,
              "confidence": 0.9998324,
              "punctuated_word": "me"
            },
            {
              "word": "more",
              "start": 6.6299996,
              "end": 6.79,
              "confidence": 0.9995466,
              "punctuated_word": "more"
            },
            {
              "word": "about",
              "start": 6.79,
              "end": 7.0299997,
              "confidence": 0.99984455,
              "punctuated_word": "about"
            },
            {
              "word": "this",
              "start": 7.0299997,
              "end": 7.2699995,
              "confidence": 0.99964225,
              "punctuated_word": "this"
            }
          ]
        }
      ]
    },
    "metadata": {
      "request_id": "52cc0efe-fa77-4aa7-b79c-0dda09de2f14",
      "model_info": {
        "name": "2-general-nova",
        "version": "2024-01-18.26916",
        "arch": "nova-2"
      },
      "model_uuid": "c0d1a568-ce81-4fea-97e7-bd45cb1fdf3c"
    },
    "from_finalize": false
  }
  ```
</CodeGroup>

In this default response, we see:

* `transcript`: the transcript for the audio segment being processed.
* `confidence`: a floating point value between 0 and 1 that indicates overall transcript reliability. Larger values indicate higher confidence.
* `words`: an object containing each `word` in the transcript, along with its `start` time and `end` time (in seconds) from the beginning of the audio stream, and a `confidence` value.
  * Because we passed the `smart_format: true` option to the `transcription.prerecorded` method, each word object also includes its `punctuated_word` value, which contains the transformed word after punctuation and capitalization are applied.
* `speech_final`: tells us this segment of speech naturally ended at this point. By default, Deepgram live streaming looks for any deviation in the natural flow of speech and returns a finalized response at these places. To learn more about this feature, see [Endpointing](/docs/endpointing/).
* `is_final`: If this says `false`, it is indicating that Deepgram will continue waiting to see if more data will improve its predictions. Deepgram live streaming can return a series of interim transcripts followed by a final transcript. To learn more, see [Interim Results](/docs/interim-results/).

<Info>
  Endpointing can be used with Deepgram's [Interim Results](/docs/interim-results/) feature. To compare and contrast these features, and to explore best practices for using them together, see [Using Endpointing and Interim Results with Live Streaming Audio](/docs/understand-endpointing-interim-results/).
</Info>

If your scenario requires you to keep the connection alive even while data is not being sent to Deepgram, you can send periodic KeepAlive messages to essentially "pause" the connection without closing it. To learn more, see [KeepAlive](/docs/audio-keep-alive).

## What's Next?

Now that you've gotten transcripts for streaming audio, enhance your knowledge by exploring the following areas. You can also check out our [Live Streaming API Reference](/reference/speech-to-text/listen-streaming) for a list of all possible parameters.

### Read the Feature Guides

Deepgram's features help you to customize your transcripts.

* [Language](/docs/language): Learn how to transcribe audio in other languages.
* [Feature Overview](/docs/stt-streaming-feature-overview): Review the list of features available for streaming speech-to-text. Then, dive into individual guides for more details.

### Tips and tricks

* [End of speech detection](/docs/understanding-end-of-speech-detection) - Learn how to pinpoint end of speech post-speaking more effectively.
* [Using interim results](/docs/using-interim-results) - Learn how to use preliminary results provided during the streaming process which can help with speech detection.
* [Measuring streaming latency](/docs/measuring-streaming-latency) - Learn how to measure latency in real-time streaming of audio.

### Add Your Audio

* Ready to connect Deepgram to your own audio source? Start by reviewing [how to determine your audio format](/docs/determining-your-audio-format-for-live-streaming-audio/) and format your API request accordingly.
* Then, check out our [Live Streaming Starter Kit](/docs/getting-started-with-the-streaming-test-suite). It's the perfect "102" introduction to integrating your own audio.

### Explore Use Cases

* Learn about the different ways you can use Deepgram products to help you meet your business objectives. [Explore Deepgram's use cases](/docs/transcribe-recorded-calls-with-twilio).

### Transcribe Pre-recorded Audio

* Now that you know how to transcribe streaming audio, check out how you can use Deepgram to transcribe pre-recorded audio. To learn more, see [Getting Started with Pre-recorded Audio](/docs/pre-recorded-audio).

***
