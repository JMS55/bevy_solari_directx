struct FullscreenVertexOutput {
    float4 clipPosition : SV_Position;
    float2 uv : TEXCOORD0;
};

FullscreenVertexOutput VSMain(uint vertexId : SV_VertexID) {
    FullscreenVertexOutput output;
    output.uv = float2(vertexId >> 1, vertexId & 1) * 2;
    output.clipPosition = float4(output.uv * float2(2, -2) + float2(-1, 1), 0, 1);
    return output;
}

float4 PSMain(FullscreenVertexOutput vertexOutput) : SV_Target {
    return float4(vertexOutput.clipPosition.xyz, 1.0);
}
