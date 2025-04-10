#include <metal_stdlib>
using namespace metal;

typedef enum AAPLVertexInputIndex
{
    AAPLVertexInputIndexViewportSize = 1,
} AAPLVertexInputIndex;

typedef struct
{
    float2 position [[attribute(0)]];
    float4 color [[attribute(1)]];
} VertexIn;

typedef struct
{
    float4 position [[position]];
    float4 color;
} RasterizerData;

vertex RasterizerData
vertexShader(VertexIn in [[stage_in]],
             constant float2& viewportSize [[buffer(AAPLVertexInputIndexViewportSize)]])
{
    RasterizerData out;
    float2 pixelSpacePosition = in.position;
    out.position = float4(0.0, 0.0, 0.0, 1.0);
    out.position.xy = pixelSpacePosition / (viewportSize / 2.0);
    out.color = in.color;
    return out;
}

fragment float4 fragmentShader(RasterizerData in [[stage_in]])
{
    return in.color;
}
