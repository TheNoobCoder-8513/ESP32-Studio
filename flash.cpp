#include <Arduino.h>

unsigned long lastTelemetryTime = 0;
const unsigned long TELEMETRY_INTERVAL = 1000; // 1000 ms = 1 sec

// Array of controllable output GPIO pins on standard ESP32 NodeMCU DevKit v1
const int OUTPUT_PINS[] = {23, 22, 1, 3, 21, 19, 18, 5, 17, 16, 4, 2, 15, 32, 33, 25, 26, 27, 14, 12, 13};
const int NUM_PINS = sizeof(OUTPUT_PINS) / sizeof(OUTPUT_PINS[0]);

void sendTelemetry() {
  // Construct JSON telemetry packet matching protocol.slint
  String json = "{";
  json += "\"type\":\"telemetry\",";
  json += "\"device\":\"ESP32-Garden\",";
  json += "\"interval\":1000,";
  json += "\"version\":1,";
  json += "\"pins\":[";

  for (int i = 0; i < NUM_PINS; i++) {
    int pin = OUTPUT_PINS[i];
    json += "{\"id\":";
    json += String(pin);
    json += ",\"status\":";
    json += (digitalRead(pin) == HIGH ? "true" : "false");
    json += "}";
    if (i < NUM_PINS - 1) {
      json += ",";
    }
  }

  json += "],";
  json += "\"vars\":[";
  json += "{\"name\":\"uptime_s\",\"value\":";
  json += String(millis() / 1000);
  json += "},";
  json += "{\"name\":\"heap_free\",\"value\":";
  json += String(ESP.getFreeHeap());
  json += "}";
  json += "]}";

  Serial.println(json);
}

void setup() {
  Serial.begin(115200); // 115200 baud recommended for JSON telemetry
  while (!Serial);

  // Initialize all controllable GPIO pins as outputs with default LOW state
  for (int i = 0; i < NUM_PINS; i++) {
    pinMode(OUTPUT_PINS[i], OUTPUT);
    digitalWrite(OUTPUT_PINS[i], LOW);
  }

  Serial.println("ESP32 GPIO Controller & Telemetry Ready!");
}

void loop() {
  // 1. Process incoming serial commands ("<pin>:on", "<pin>:off")
  if (Serial.available() > 0) {
    String cmd = Serial.readStringUntil('\n');
    cmd.trim();

    int sep = cmd.indexOf(':');
    if (sep != -1) {
      int pin = cmd.substring(0, sep).toInt();
      String state = cmd.substring(sep + 1);
      state.trim();

      pinMode(pin, OUTPUT);
      if (state.equalsIgnoreCase("on") || state == "1") {
        digitalWrite(pin, HIGH);
        Serial.print("Success: Pin ");
        Serial.print(pin);
        Serial.println(" is now ON (HIGH)");
      } else if (state.equalsIgnoreCase("off") || state == "0") {
        digitalWrite(pin, LOW);
        Serial.print("Success: Pin ");
        Serial.print(pin);
        Serial.println(" is now OFF (LOW)");
      } else {
        Serial.println("Error: Unknown state. Use 'on' or 'off'.");
      }
    }
  }

  // 2. Non-blocking 1-second telemetry transmission
  if (millis() - lastTelemetryTime >= TELEMETRY_INTERVAL) {
    lastTelemetryTime = millis();
    sendTelemetry();
  }
}
